//! Safe wrappers over the Win32 calls ccdash makes on Windows.

use windows_sys::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};

/// The DNS host name, as configured.
pub fn hostname() -> Option<String> {
    let mut len: u32 = 0;
    // SAFETY: a null buffer asks for the length in UTF-16 units, terminator included
    unsafe { GetComputerNameExW(ComputerNameDnsHostname, std::ptr::null_mut(), &raw mut len) };
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` holds `len` units; on success `len` is the name's length without the terminator
    if unsafe { GetComputerNameExW(ComputerNameDnsHostname, buf.as_mut_ptr(), &raw mut len) } == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}
