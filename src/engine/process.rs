use anyhow::{Result, bail};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::{Output, Stdio};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::process::{Child, Command};

// TODO: Cleanup time after SIGTERM is unpredictable, so 30s is a policy choice,
// not a measured value. Revisit it if it proves too short or too long in practice.
const TERM_GRACE: Duration = Duration::from_secs(30);

static PENDING_KILLS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

pub struct ChildGuard {
    pidfd: Option<OwnedFd>,
}

impl ChildGuard {
    pub fn new(child: &Child) -> Option<Self> {
        let pid = child.id()? as i32;
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            return None;
        }
        Some(Self {
            pidfd: Some(unsafe { OwnedFd::from_raw_fd(fd as i32) }),
        })
    }
}

fn send_signal(pidfd: i32, sig: i32) {
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd,
            sig,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        );
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let Some(pidfd) = self.pidfd.take() else {
            return;
        };
        send_signal(pidfd.as_raw_fd(), libc::SIGTERM);
        // A detached thread would die with the process, so finish_pending_kills joins it.
        let handle = std::thread::spawn(move || {
            let mut pfd = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let timed_out = unsafe { libc::poll(&mut pfd, 1, TERM_GRACE.as_millis() as i32) } == 0;
            if timed_out {
                send_signal(pidfd.as_raw_fd(), libc::SIGKILL);
            }
        });
        PENDING_KILLS.lock().unwrap().push(handle);
    }
}

pub fn finish_pending_kills() {
    let handles: Vec<_> = PENDING_KILLS.lock().unwrap().drain(..).collect();
    for handle in handles {
        let _ = handle.join();
    }
}

pub async fn run_checked(mut cmd: Command, err_context: &str) -> Result<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn()?;

    let _guard = ChildGuard::new(&child);
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
