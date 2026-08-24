//! The Linux implementation of namespace isolation: `unshare`, `pivot_root`,
//! loopback bring-up, and the signal-forwarding shim.
//!
//! See the parent module for why this runs from `main` rather than `pre_exec`.

use libc::{c_char, c_int, c_short};
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, pivot_root, ForkResult, Pid};
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};

/// PID of the inner (namespace-init) process. Written by the outer shim
/// before installing signal handlers so the handler can relay signals.
/// `0` means "no inner process to forward to yet".
static INNER_PID: AtomicI32 = AtomicI32::new(0);

/// Unshare mount/net/pid namespaces, confine the filesystem, then fork so the
/// worker becomes PID 1 of the new PID namespace. Never returns on success.
pub(super) fn exec_isolated(root: &Path, command: &str) -> io::Result<()> {
    // CLONE_NEWNS / CLONE_NEWNET move the *calling* process directly
    // (unlike CLONE_NEWPID, see module docs), so no fork is needed for
    // these two.
    unshare(CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWNET).map_err(to_io_err)?;

    bring_up_loopback()?;
    isolate_mount_namespace(root)?;

    // CLONE_NEWPID only takes effect for children created after this call
    // returns, so the worker itself must be such a child.
    unshare(CloneFlags::CLONE_NEWPID).map_err(to_io_err)?;

    match unsafe { fork() }.map_err(to_io_err)? {
        ForkResult::Child => {
            // PID 1 of the new namespace. `exec` replaces this process's
            // image entirely; it only returns here if the exec itself
            // failed.
            Err(std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .exec())
        }
        ForkResult::Parent { child } => {
            // Never returns: becomes the signal-forwarding shim.
            run_signal_forwarding_shim(child);
        }
    }
}

fn to_io_err(e: nix::Error) -> io::Error {
    io::Error::from_raw_os_error(e as i32)
}

/// Bring the loopback interface up inside the new network namespace using
/// raw ioctls only.
fn bring_up_loopback() -> io::Result<()> {
    const IFNAMSIZ: usize = 16;

    #[repr(C)]
    struct IfReq {
        ifr_name: [c_char; IFNAMSIZ],
        ifr_flags: c_short,
        _padding: [u8; 22],
    }

    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut req: IfReq = unsafe { std::mem::zeroed() };
    for (i, b) in b"lo\0".iter().enumerate() {
        req.ifr_name[i] = *b as c_char;
    }

    let result = (|| -> io::Result<()> {
        if unsafe { libc::ioctl(sock, libc::SIOCGIFFLAGS as _, &mut req) } < 0 {
            return Err(io::Error::last_os_error());
        }

        req.ifr_flags |= libc::IFF_UP as c_short;

        if unsafe { libc::ioctl(sock, libc::SIOCSIFFLAGS as _, &req) } < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    })();

    unsafe { libc::close(sock) };
    result
}

/// Restrict the worker's filesystem view to `root` via `pivot_root`.
///
/// Follows the standard safe `pivot_root(2)` recipe:
/// 1. Make the mount namespace private (`MS_REC|MS_PRIVATE` on `/`) so
///    mount/unmount events here never propagate back to the host.
/// 2. Bind-mount `root` onto itself so it is a mount point, `pivot_root`
///    requires the new root to be a mount point, not just a directory.
/// 3. Create `root/.riku_old_root`, pivot, `chdir` to `/`, then detach and
///    remove the old root so the host filesystem is unreachable.
fn isolate_mount_namespace(root: &Path) -> io::Result<()> {
    mount(
        Some("/"),
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )
    .map_err(to_io_err)?;

    mount(
        Some(root),
        root,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(to_io_err)?;

    let old_root = root.join(".riku_old_root");
    std::fs::create_dir_all(&old_root)?;

    pivot_root(root, &old_root).map_err(to_io_err)?;

    nix::unistd::chdir("/").map_err(to_io_err)?;

    // The old root is now mounted at /.riku_old_root; detach it so the
    // worker has no path back to the host filesystem.
    umount2("/.riku_old_root", MntFlags::MNT_DETACH).map_err(to_io_err)?;
    let _ = std::fs::remove_dir("/.riku_old_root");

    Ok(())
}

/// Forward termination signals to `child` and relay its exit status by
/// calling `_exit` with a matching code. Never returns.
fn run_signal_forwarding_shim(child: Pid) -> ! {
    INNER_PID.store(child.as_raw(), Ordering::SeqCst);

    // SAFETY: forward_signal performs only an atomic load and a kill()
    // syscall: both async-signal-safe.
    extern "C" fn forward_signal(sig: c_int) {
        let pid = INNER_PID.load(Ordering::SeqCst);
        if pid > 0 {
            unsafe {
                libc::kill(pid, sig);
            }
        }
    }

    for signal in [
        Signal::SIGTERM,
        Signal::SIGINT,
        Signal::SIGHUP,
        Signal::SIGQUIT,
    ] {
        let action = SigAction::new(
            SigHandler::Handler(forward_signal),
            SaFlags::empty(),
            SigSet::empty(),
        );
        unsafe {
            let _ = sigaction(signal, &action);
        }
    }

    loop {
        match waitpid(child, None) {
            Ok(WaitStatus::Exited(_, code)) => unsafe { libc::_exit(code) },
            Ok(WaitStatus::Signaled(_, sig, _)) => unsafe { libc::_exit(128 + sig as i32) },
            Ok(_) => continue,
            Err(nix::errno::Errno::EINTR) => continue,
            Err(_) => unsafe { libc::_exit(1) },
        }
    }
}
