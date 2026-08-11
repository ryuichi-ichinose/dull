use anyhow::{Result, bail};
use std::process::{Output, Stdio};
use tokio::process::{Child, Command};

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

pub async fn run_checked(mut cmd: Command, err_context: &str) -> Result<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn()?;

    let _guard = ProcessGroupGuard::new(&child);
    let output = child.wait_with_output().await?;

    if !output.status.success() {
        bail!(
            "{}:\n{}",
            err_context,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(output)
}
