use anyhow::{Context, Result, bail};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;
pub struct BuildLock {
    _file: File,
}

impl BuildLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let file =
            File::create(path).with_context(|| format!("Failed to open lock file {:?}", path))?;

        let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
                bail!(
                    "Another 'dull build' is already running in this project (lock held on {:?}).",
                    path
                );
            }
            return Err(err).context("Failed to acquire build lock");
        }

        Ok(Self { _file: file })
    }
}
