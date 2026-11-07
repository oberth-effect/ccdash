//! Safe wrappers over the Win32 calls ccdash makes on Windows.

use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    time::Duration,
};

use windows_sys::{
    Win32::{
        Foundation::{
            ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, ERROR_SUCCESS, HANDLE, LocalFree,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
            },
            EqualSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        Storage::FileSystem::{ReadFile, WriteFile},
        System::{
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Pipes::WaitNamedPipeW,
            SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW},
            Threading::{
                CreateEventW, GetCurrentProcess, INFINITE, OpenProcessToken, WaitForSingleObject,
            },
        },
    },
    core::{BOOL, PWSTR},
};

/// The DNS host name, as configured.
pub fn hostname() -> Option<String> {
    let mut len: u32 = 0;
    // SAFETY: a null buffer asks for the length in UTF-16 units, terminator included
    unsafe { GetComputerNameExW(ComputerNameDnsHostname, ptr::null_mut(), &raw mut len) };
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` holds `len` units; on success `len` is the name's length without the terminator
    if unsafe { GetComputerNameExW(ComputerNameDnsHostname, buf.as_mut_ptr(), &raw mut len) } == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

/// `s` as a NUL-terminated UTF-16 string.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// A NUL-terminated UTF-16 string at `p`.
///
/// # Safety
///
/// `p` points at a NUL-terminated UTF-16 string.
unsafe fn from_wide(p: *const u16) -> String {
    let mut len = 0;
    // SAFETY: the caller promises a terminator
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units were just read
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

fn millis(timeout: Option<Duration>) -> u32 {
    timeout.map_or(INFINITE, |t| {
        u32::try_from(t.as_millis()).map_or(INFINITE - 1, |ms| ms.clamp(1, INFINITE - 1))
    })
}

/// A manual-reset event for overlapped I/O.
pub struct Event(OwnedHandle);

impl Event {
    pub fn new() -> io::Result<Self> {
        // SAFETY: no attributes, no name; manual reset, not signalled
        let handle = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a fresh handle nothing else owns
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }
}

/// One read on `file` (opened with `FILE_FLAG_OVERLAPPED`), given up after
/// `timeout`.
pub fn read_overlapped(
    file: &impl AsRawHandle,
    event: &Event,
    buf: &mut [u8],
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let handle = file.as_raw_handle();
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    let data = buf.as_mut_ptr();
    // SAFETY: `buf` outlives the operation: `overlapped` waits for it to end
    overlapped(handle, event, timeout, |ov| unsafe {
        ReadFile(handle, data, len, ptr::null_mut(), ov)
    })
}

/// One write on `file` (opened with `FILE_FLAG_OVERLAPPED`), given up
/// after `timeout`.
pub fn write_overlapped(
    file: &impl AsRawHandle,
    event: &Event,
    buf: &[u8],
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let handle = file.as_raw_handle();
    let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    let data = buf.as_ptr();
    // SAFETY: `buf` outlives the operation: `overlapped` waits for it to end
    overlapped(handle, event, timeout, |ov| unsafe {
        WriteFile(handle, data, len, ptr::null_mut(), ov)
    })
}

/// Starts an operation with `start` and waits for it; when the wait times
/// out or fails it is cancelled, and still waited for, since it holds
/// `OVERLAPPED`: it may have completed meanwhile, and then counts.
fn overlapped(
    handle: HANDLE,
    event: &Event,
    timeout: Option<Duration>,
    start: impl FnOnce(*mut OVERLAPPED) -> BOOL,
) -> io::Result<usize> {
    // SAFETY: all-zero is a valid OVERLAPPED
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.hEvent = event.0.as_raw_handle();
    let mut cut_short = None;
    if start(&raw mut ov) == 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
            return Err(e);
        }
        // SAFETY: the event belongs to `event`, alive here
        match unsafe { WaitForSingleObject(ov.hEvent, millis(timeout)) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => cut_short = Some(io::ErrorKind::TimedOut.into()),
            _ => cut_short = Some(io::Error::last_os_error()),
        }
        if cut_short.is_some() {
            // SAFETY: `ov` is the pending operation's; the wait below outlives it
            unsafe { CancelIoEx(handle, &raw const ov) };
        }
    }
    let mut done = 0;
    // SAFETY: the operation of `ov` has ended, or ends (cancelled) while this waits
    if unsafe { GetOverlappedResult(handle, &raw const ov, &raw mut done, 1) } == 0 {
        let e = io::Error::last_os_error();
        return Err(match cut_short {
            Some(why) if e.raw_os_error() == Some(ERROR_OPERATION_ABORTED as i32) => why,
            _ => e,
        });
    }
    Ok(done as usize)
}

/// Waits up to `timeout` for an instance of the pipe `name` to be free.
pub fn wait_named_pipe(name: &str, timeout: Duration) -> bool {
    let name = wide(name);
    // SAFETY: `name` is NUL-terminated
    unsafe { WaitNamedPipeW(name.as_ptr(), millis(Some(timeout))) != 0 }
}

/// Security attributes that let only the current user in, and make the
/// user the owner (an elevated token would make it Administrators).
pub struct OwnerOnly {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: SECURITY_ATTRIBUTES,
}

// SAFETY: the descriptor is heap memory only this value refers to and frees
unsafe impl Send for OwnerOnly {}

impl OwnerOnly {
    pub fn new() -> io::Result<Self> {
        let sid = user_sid()?;
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})"));
        let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated; the descriptor is LocalFree'd on drop
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &raw mut descriptor,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            descriptor,
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        })
    }

    /// The `SECURITY_ATTRIBUTES`, valid while `self` is.
    pub fn as_ptr(&self) -> *mut c_void {
        (&raw const self.attributes).cast_mut().cast()
    }
}

impl Drop for OwnerOnly {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW
        unsafe { LocalFree(self.descriptor) };
    }
}

/// Whether the current user owns the kernel object behind `handle` (it
/// needs `READ_CONTROL`).
pub fn owned_by_me(handle: &impl AsRawHandle) -> io::Result<bool> {
    let mut owner: PSID = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: a live handle; `owner` points into `descriptor`, LocalFree'd below
    let status = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let same = token_user().map(|buf| {
        // SAFETY: token_user() wrote a TOKEN_USER at the start of `buf`
        let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
        // SAFETY: both SIDs are valid while `descriptor` and `buf` live
        unsafe { EqualSid(owner, user.User.Sid) != 0 }
    });
    // SAFETY: allocated by GetSecurityInfo
    unsafe { LocalFree(descriptor) };
    same
}

/// The current user's SID, as `S-1-5-…`.
fn user_sid() -> io::Result<String> {
    let buf = token_user()?;
    // SAFETY: token_user() wrote a TOKEN_USER at the start of `buf`
    let user = unsafe { &*buf.as_ptr().cast::<TOKEN_USER>() };
    let mut text: PWSTR = ptr::null_mut();
    // SAFETY: the SID lives in `buf`; `text` is LocalFree'd below
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ConvertSidToStringSidW returned a NUL-terminated string
    let sid = unsafe { from_wide(text) };
    // SAFETY: allocated by ConvertSidToStringSidW
    unsafe { LocalFree(text.cast()) };
    Ok(sid)
}

/// The current process's `TOKEN_USER`, at the start of the buffer.
fn token_user() -> io::Result<Vec<u64>> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: the pseudo handle needs no closing; `token` receives a real one
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a fresh handle nothing else owns
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut len = 0u32;
    // SAFETY: a null buffer asks for the size
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &raw mut len,
        );
    }
    // NOTE: u64s, so the TOKEN_USER read from it is aligned
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buf.as_mut_ptr().cast(),
            len,
            &raw mut len,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_objects_of_other_owners() {
        let system = std::env::var_os("SystemRoot").unwrap();
        // NOTE: TrustedInstaller owns the system's executables
        let notepad =
            std::fs::File::open(std::path::Path::new(&system).join(r"System32\notepad.exe"))
                .unwrap();
        assert!(!owned_by_me(&notepad).unwrap());
    }
}
