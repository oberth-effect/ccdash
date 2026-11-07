//! Safe wrappers over the Win32 calls ccdash makes on Windows.

use std::{
    ffi::c_void,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr,
    time::Duration,
};

use windows_sys::{
    Wdk::System::Threading::NtQueryInformationProcess,
    Wdk::System::Threading::PROCESSINFOCLASS,
    Wdk::System::Threading::ProcessBasicInformation,
    Win32::{
        Foundation::{
            ERROR_ACCESS_DENIED, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, ERROR_SUCCESS,
            FILETIME, HANDLE, INVALID_HANDLE_VALUE, LocalFree, UNICODE_STRING, WAIT_FAILED,
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
            Diagnostics::ToolHelp::CreateToolhelp32Snapshot,
            Diagnostics::ToolHelp::PROCESSENTRY32W,
            Diagnostics::ToolHelp::Process32FirstW,
            Diagnostics::ToolHelp::Process32NextW,
            Diagnostics::ToolHelp::TH32CS_SNAPPROCESS,
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Pipes::WaitNamedPipeW,
            ProcessStatus::K32GetProcessMemoryInfo,
            ProcessStatus::PROCESS_MEMORY_COUNTERS,
            SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW},
            Threading::{
                CREATE_NEW_CONSOLE, CreateEventW, CreateProcessW, GetCurrentProcess,
                GetExitCodeProcess, GetProcessTimes, INFINITE, OpenProcess, OpenProcessToken,
                PROCESS_BASIC_INFORMATION, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, PROCESS_VM_READ,
                QueryFullProcessImageNameW, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
            },
        },
        UI::Shell::CommandLineToArgvW,
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

/// A process in the system's process list.
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    /// The executable's file name.
    pub exe: String,
}

/// Every process, from a Toolhelp snapshot.
pub fn processes() -> Vec<Process> {
    // SAFETY: plain call; the handle is owned below
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    // SAFETY: a fresh handle nothing else owns
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    // SAFETY: all-zero is a valid PROCESSENTRY32W
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut out = Vec::new();
    // SAFETY: `entry` is a PROCESSENTRY32W with dwSize set
    let mut more = unsafe { Process32FirstW(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while more {
        let name = &entry.szExeFile;
        let len = name.iter().position(|c| *c == 0).unwrap_or(name.len());
        out.push(Process {
            pid: entry.th32ProcessID,
            ppid: entry.th32ParentProcessID,
            exe: String::from_utf16_lossy(&name[..len]),
        });
        // SAFETY: as above
        more = unsafe { Process32NextW(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    out
}

fn open_process(pid: u32, access: u32) -> Option<OwnedHandle> {
    // SAFETY: plain call; null on failure
    let handle = unsafe { OpenProcess(access, 0, pid) };
    // SAFETY: a fresh handle nothing else owns
    (!handle.is_null()).then(|| unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// 100 ns intervals between 1601-01-01 and 1970-01-01.
const EPOCH_DIFF: u64 = 116_444_736_000_000_000;

fn filetime(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

/// When a process started and the CPU time it used.
pub struct Times {
    /// Milliseconds since the Unix epoch.
    pub started_ms: i64,
    /// Kernel plus user time, in 100 ns units.
    pub cpu_100ns: u64,
}

pub fn process_times(pid: u32) -> Option<Times> {
    let process = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four FILETIMEs to fill, a live handle
    let ok = unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &raw mut created,
            &raw mut exited,
            &raw mut kernel,
            &raw mut user,
        )
    };
    (ok != 0).then(|| Times {
        started_ms: (filetime(created).saturating_sub(EPOCH_DIFF) / 10_000) as i64,
        cpu_100ns: filetime(kernel) + filetime(user),
    })
}

/// The working set of a process, in bytes.
pub fn process_memory(pid: u32) -> Option<u64> {
    let process = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ)
        .or_else(|| open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION))?;
    // SAFETY: all-zero is a valid PROCESS_MEMORY_COUNTERS
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    let size = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: `counters` has `size` bytes, a live handle
    let ok = unsafe { K32GetProcessMemoryInfo(process.as_raw_handle(), &raw mut counters, size) };
    (ok != 0).then_some(counters.WorkingSetSize as u64)
}

/// The pid that started `pid` (it may be gone, its pid another
/// process's by now).
pub fn parent_of(pid: u32) -> Option<u32> {
    let process = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    // SAFETY: all-zero is a valid PROCESS_BASIC_INFORMATION
    let mut info: PROCESS_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: a live handle; `info` has the size given
    let status = unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            ProcessBasicInformation,
            (&raw mut info).cast(),
            size_of::<PROCESS_BASIC_INFORMATION>() as u32,
            ptr::null_mut(),
        )
    };
    if status < 0 {
        return None;
    }
    u32::try_from(info.InheritedFromUniqueProcessId).ok()
}

/// The file name of the executable `pid` runs.
pub fn exe_name(pid: u32) -> Option<String> {
    let process = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let mut buf = vec![0u16; 32_768];
    let mut len = buf.len() as u32;
    // SAFETY: a live handle; `buf` holds `len` units
    if unsafe {
        QueryFullProcessImageNameW(process.as_raw_handle(), 0, buf.as_mut_ptr(), &raw mut len)
    } == 0
    {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Some(path.rsplit(['\\', '/']).next().unwrap_or(&path).to_owned())
}

/// `ProcessCommandLineInformation`: the command line as a UNICODE_STRING.
const PROCESS_COMMAND_LINE_INFORMATION: PROCESSINFOCLASS = 60;

/// The command line a process was started with.
pub fn command_line(pid: u32) -> Option<String> {
    let process = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let mut len = 0u32;
    // SAFETY: a zero-length buffer asks for the size
    unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            PROCESS_COMMAND_LINE_INFORMATION,
            ptr::null_mut(),
            0,
            &raw mut len,
        )
    };
    if len == 0 {
        return None;
    }
    // NOTE: u64s, so the UNICODE_STRING read from it is aligned
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes
    let status = unsafe {
        NtQueryInformationProcess(
            process.as_raw_handle(),
            PROCESS_COMMAND_LINE_INFORMATION,
            buf.as_mut_ptr().cast(),
            len,
            &raw mut len,
        )
    };
    if status < 0 {
        return None;
    }
    // SAFETY: the call wrote a UNICODE_STRING at the start of `buf`, its text after it in `buf`
    let text = unsafe {
        let s = &*buf.as_ptr().cast::<UNICODE_STRING>();
        if s.Buffer.is_null() {
            return Some(String::new());
        }
        std::slice::from_raw_parts(s.Buffer, usize::from(s.Length) / 2)
    };
    Some(String::from_utf16_lossy(text))
}

/// A command line split into arguments, as programs' C runtimes split it.
pub fn split_command_line(line: &str) -> Vec<String> {
    if line.trim().is_empty() {
        return Vec::new();
    }
    let wide_line = wide(line);
    let mut count = 0;
    // SAFETY: `wide_line` is NUL-terminated; the result is LocalFree'd below
    let argv = unsafe { CommandLineToArgvW(wide_line.as_ptr(), &raw mut count) };
    if argv.is_null() {
        return vec![line.to_owned()];
    }
    let args = (0..usize::try_from(count).unwrap_or(0))
        // SAFETY: `argv` holds `count` NUL-terminated strings
        .map(|i| unsafe { from_wide(*argv.add(i)) })
        .collect();
    // SAFETY: allocated by CommandLineToArgvW
    unsafe { LocalFree(argv.cast()) };
    args
}

/// A process in a console of its own whose standard handles are that
/// console's: std's `Command` always hands a child handles of its own.
pub struct ConsoleChild {
    process: OwnedHandle,
    pub pid: u32,
}

impl ConsoleChild {
    /// Starts `program` ([`find_program`]; a batch file through `cmd.exe`)
    /// with `args` in a new console.
    pub fn spawn(program: &str, args: &[String]) -> io::Result<Self> {
        Self::spawn_with(program, args, CREATE_NEW_CONSOLE)
    }

    fn spawn_with(program: &str, args: &[String], flags: u32) -> io::Result<Self> {
        let mut exe = find_program(program).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("{program} is not on PATH"))
        })?;
        let line = if is_batch(&exe) {
            let line = format!("cmd.exe {}", batch_arguments(&exe, args)?);
            exe = command_processor();
            line
        } else {
            let mut line = quote_arg(program);
            for arg in args {
                line.push(' ');
                line.push_str(&quote_arg(arg));
            }
            line
        };
        let exe: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut line = wide(&line);
        // SAFETY: all-zero is a valid STARTUPINFOW and PROCESS_INFORMATION
        let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
        startup.cb = size_of::<STARTUPINFOW>() as u32;
        // SAFETY: as above
        let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // NOTE: no STARTF_USESTDHANDLES and no inherited handles: the new console's are used
        // SAFETY: NUL-terminated strings, a mutable command line, structs to fill
        let ok = unsafe {
            CreateProcessW(
                exe.as_ptr(),
                line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                flags,
                ptr::null(),
                ptr::null(),
                &raw const startup,
                &raw mut info,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fresh handles nothing else owns; the thread's is closed at once
        let process = unsafe {
            drop(OwnedHandle::from_raw_handle(info.hThread));
            OwnedHandle::from_raw_handle(info.hProcess)
        };
        Ok(Self {
            process,
            pid: info.dwProcessId,
        })
    }

    /// The exit code once the process has ended; none while it runs.
    pub fn try_wait(&self) -> io::Result<Option<u32>> {
        // SAFETY: a live process handle; a zero wait only looks
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => return Ok(None),
            WAIT_FAILED => return Err(io::Error::last_os_error()),
            _ => {}
        }
        let mut code = 0u32;
        // SAFETY: as above
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &raw mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Some(code))
    }
}

/// What a program without an extension may be, in this order: npm and
/// other installers leave `.cmd` shims.
const PROGRAM_EXTENSIONS: &[&str] = &["exe", "com", "cmd", "bat"];

/// `program` itself when it is a path, else the first match on PATH; one of
/// `PROGRAM_EXTENSIONS` is added when it has no extension.
pub fn find_program(program: &str) -> Option<std::path::PathBuf> {
    find_in(program, &std::env::var_os("PATH").unwrap_or_default())
}

fn find_in(program: &str, path_var: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(program);
    let names: Vec<std::path::PathBuf> = if path.extension().is_some() {
        vec![path.to_path_buf()]
    } else {
        PROGRAM_EXTENSIONS
            .iter()
            .map(|ext| path.with_extension(ext))
            .collect()
    };
    // NOTE: an app execution alias (WindowsApps) cannot be followed, only seen
    let exists = |p: &std::path::Path| p.symlink_metadata().is_ok_and(|m| !m.is_dir());
    if program.contains(['/', '\\', ':']) {
        return names.into_iter().find(|p| exists(p));
    }
    std::env::split_paths(path_var)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|p| exists(p))
}

fn is_batch(exe: &std::path::Path) -> bool {
    exe.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// `cmd.exe`'s arguments that run the batch file `exe` with `args`.
/// Arguments `cmd` would expand or split (`"`, `%`, line breaks) are refused.
fn batch_arguments(exe: &std::path::Path, args: &[String]) -> io::Result<String> {
    let mut inner = format!("\"{}\"", exe.display());
    for arg in args {
        if arg.contains(['"', '%', '\r', '\n']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{arg:?} cannot be passed to a batch file"),
            ));
        }
        // NOTE: the program behind the shim splits as the C runtime does: trailing backslashes double
        let trailing = arg.len() - arg.trim_end_matches('\\').len();
        inner.push_str(" \"");
        inner.push_str(arg);
        inner.extend(std::iter::repeat_n('\\', trailing));
        inner.push('"');
    }
    // NOTE: /s: the outer quotes go, the rest runs as written; /v:OFF: `!` stays literal
    Ok(format!("/d /e:ON /v:OFF /s /c \"{inner}\""))
}

/// `cmd.exe`: `ComSpec`, else the system's.
fn command_processor() -> std::path::PathBuf {
    std::env::var_os("ComSpec").map_or_else(
        || {
            let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
            std::path::Path::new(&root).join(r"System32\cmd.exe")
        },
        Into::into,
    )
}

/// `arg` quoted for a command line as the C runtime splits it.
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
            continue;
        }
        // NOTE: backslashes double only before a quote
        let n = if c == '"' {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        out.extend(std::iter::repeat_n('\\', n));
        backslashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

/// Whether a process with this id runs. One this user may not open
/// (elevated, another user's, protected) runs too, as `/proc/<pid>` exists
/// for another user's; a gone pid is `ERROR_INVALID_PARAMETER`.
pub fn pid_alive(pid: u32) -> bool {
    // SAFETY: plain call; null on failure
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED as i32);
    }
    // SAFETY: a fresh handle nothing else owns
    let process = unsafe { OwnedHandle::from_raw_handle(handle) };
    // SAFETY: a live handle with SYNCHRONIZE; a zero wait only looks
    unsafe { WaitForSingleObject(process.as_raw_handle(), 0) == WAIT_TIMEOUT }
}

/// Ends a process at once (`TerminateProcess`); it gets no say.
pub fn terminate(pid: u32) -> io::Result<()> {
    let process = open_process(pid, PROCESS_TERMINATE).ok_or_else(io::Error::last_os_error)?;
    // SAFETY: a live handle with PROCESS_TERMINATE
    if unsafe { TerminateProcess(process.as_raw_handle(), 1) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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
    fn processes_this_user_may_not_open_still_run() {
        // NOTE: the System process (4) refuses an unelevated user's SYNCHRONIZE
        assert!(pid_alive(4));
        assert!(!pid_alive(0), "the idle process is not one");
    }

    fn ended(child: &ConsoleChild) -> u32 {
        loop {
            if let Some(code) = child.try_wait().unwrap() {
                return code;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn quotes_arguments_as_they_split() {
        let args = [
            "plain",
            "with space",
            r#"quo"te"#,
            r"trail\",
            r#"back\"q"#,
            "",
            r"C:\a b\",
        ]
        .map(String::from);
        let line = std::iter::once("prog".to_owned())
            .chain(args.iter().map(|a| quote_arg(a)))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(split_command_line(&line)[1..], args);
    }

    #[test]
    fn programs_are_found_as_exe_or_batch_and_batch_files_run() {
        let dir = std::env::temp_dir().join(format!("ccdash-prog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("ccdash-shim.cmd");
        std::fs::write(&shim, "@exit /b %~1\r\n").unwrap();
        let path_var = std::env::join_paths([dir.clone()]).unwrap();
        assert_eq!(find_in("ccdash-shim", &path_var), Some(shim.clone()));
        assert_eq!(find_in("ccdash-none", &path_var), None);
        assert!(find_program("cmd").is_some_and(|p| p.ends_with("cmd.exe")));
        let child = ConsoleChild::spawn_with(
            shim.to_str().unwrap(),
            &["7".into()],
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW,
        )
        .unwrap();
        assert_eq!(ended(&child), 7);
        // NOTE: std's Command runs a batch file it is given the path of
        let status = {
            use std::os::windows::process::CommandExt;
            std::process::Command::new(&shim)
                .arg("5")
                .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
                .status()
                .unwrap()
        };
        assert_eq!(status.code(), Some(5));
        assert!(batch_arguments(&shim, &["100%".into()]).is_err());
        assert_eq!(
            batch_arguments(&shim, &[r"C:\a b\".into()]).unwrap(),
            format!(r#"/d /e:ON /v:OFF /s /c ""{}" "C:\a b\\"""#, shim.display())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_console_child_has_its_console_for_input() {
        let script = "exit ([int][Console]::IsInputRedirected + 7)";
        let args = ["-NoProfile", "-Command", script].map(String::from);
        // NOTE: CREATE_NO_WINDOW: a console of its own without a window on the desktop
        let child = ConsoleChild::spawn_with(
            "powershell",
            &args,
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW,
        )
        .unwrap();
        assert_eq!(ended(&child), 7, "stdin is the console, not redirected");
        assert!(ConsoleChild::spawn("ccdash-no-such-program", &[]).is_err());
    }

    #[test]
    fn this_process_is_listed_with_its_times_and_command_line() {
        let me = std::process::id();
        let listed = processes().into_iter().find(|p| p.pid == me).unwrap();
        assert!(
            listed.exe.to_ascii_lowercase().ends_with(".exe"),
            "{}",
            listed.exe
        );
        assert!(pid_alive(me) && pid_alive(listed.ppid));
        assert_eq!(parent_of(me), Some(listed.ppid));
        assert!(exe_name(me).is_some_and(|exe| exe.eq_ignore_ascii_case(&listed.exe)));
        let times = process_times(me).unwrap();
        let age_ms = crate::util::now_ms() - times.started_ms;
        assert!((0..600_000).contains(&age_ms), "{age_ms}");
        assert!(process_memory(me).unwrap() > 0);
        let args = split_command_line(&command_line(me).unwrap());
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_stem().unwrap().to_string_lossy();
        assert!(args[0].contains(name.as_ref()), "{args:?}");
    }

    #[test]
    fn command_lines_split_as_the_c_runtime_does() {
        assert_eq!(
            split_command_line(r#""C:\Program Files\x.exe" -c "a b" c"#),
            [r"C:\Program Files\x.exe", "-c", "a b", "c"]
        );
        assert!(split_command_line(" ").is_empty());
    }

    #[test]
    fn a_child_ends_when_terminated() {
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 30 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        assert!(pid_alive(child.id()));
        terminate(child.id()).unwrap();
        assert!(!child.wait().unwrap().success());
        assert!(!pid_alive(child.id()));
    }

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
