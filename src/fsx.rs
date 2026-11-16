//! Owner-only file modes. Unix applies them; on Windows `set_mode` gives an
//! owner-only mode (no group or other bits) a DACL that lets only the user
//! in, and a file created with a mode takes the DACL of its directory,
//! which `set_mode` made owner-only (the state, runtime and edit dirs).
//! Also a directory's time, which Windows sets through a handle of its own
//! kind.

use std::{fs::OpenOptions, io, path::Path, time::SystemTime};

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

/// Sets the permission bits of `path` on Unix; on Windows an owner-only
/// mode becomes an owner-only DACL, any other leaves the DACL alone.
///
/// # Errors
///
/// `chmod` (or setting the DACL) failed.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        if mode & 0o077 == 0 {
            crate::win::owner_only(path)?;
        }
        Ok(())
    }
}

/// Sets the modification time of the directory `dir`.
///
/// # Errors
///
/// The directory cannot be opened, or its time cannot be set.
pub fn set_dir_modified(dir: &Path, time: SystemTime) -> io::Result<()> {
    #[cfg(unix)]
    let opened = std::fs::File::open(dir);
    // NOTE: Windows opens a directory only with backup semantics
    #[cfg(windows)]
    let opened = {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES,
        };
        OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)
    };
    opened?.set_modified(time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_takes_a_time() {
        let dir = std::env::temp_dir().join(format!("ccdash-fsx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let then = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        set_dir_modified(&dir, then).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().modified().unwrap(), then);
        assert!(set_dir_modified(&dir.join("missing"), then).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
