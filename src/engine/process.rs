use tokio::process::Child;

pub struct ProcessGroupGuard {
    pgid: i32,
}

impl ProcessGroupGuard {
    pub fn new(child: &Child) -> Option<Self> {
        child.id().map(|pid| Self { pgid: pid as i32 })
    }
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-self.pgid, libc::SIGKILL);
        }
    }
}
