//! Owner-only file modes. Unix applies them; on Windows a file takes the
//! ACL of the directory it is created in (under the user's profile), so
//! they do nothing there.

use std::{fs::OpenOptions, io, path::Path};

/// `OpenOptions::mode` on Unix; nothing elsewhere.
pub trait ModeExt {
    fn mode(&mut self, mode: u32) -> &mut Self;
}

impl ModeExt for OpenOptions {
    #[cfg(unix)]
    fn mode(&mut self, mode: u32) -> &mut Self {
        std::os::unix::fs::OpenOptionsExt::mode(self, mode)
    }

    #[cfg(not(unix))]
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}

/// Sets the permission bits of `path` on Unix; nothing elsewhere.
///
/// # Errors
///
/// `chmod` failed.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}
