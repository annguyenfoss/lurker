//! Kernel process birth identity; directory-backend availability is gated separately.
use super::{process_query_absent, Process};
use crate::{AppError, AppResult};
use std::mem::{size_of, MaybeUninit};
use std::ptr::addr_of;

pub(super) fn snapshot(pid: u32) -> AppResult<Option<Process>> {
    let native_pid = i32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or_else(|| AppError::new("Invalid native backend process ID."))?;
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROC2,
        libc::KERN_PROC_PID,
        native_pid,
        size_of::<libc::kinfo_proc2>() as i32,
        1,
    ];
    for attempt in 0..3 {
        let mut info = MaybeUninit::<libc::kinfo_proc2>::uninit();
        let mut length = size_of::<libc::kinfo_proc2>();
        // The kernel writes the native ABI into this aligned storage. Read only
        // the validated primitive fields below, never an entire Rust struct.
        let result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as u32,
                info.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        let error = (result != 0)
            .then(|| std::io::Error::last_os_error().raw_os_error())
            .flatten();
        if process_query_absent(result, length, error) {
            return Ok(None);
        }
        if result != 0 && error == Some(libc::EINTR) && attempt < 2 {
            continue;
        }
        if result != 0 {
            return Err(AppError::io(
                "Cannot verify native backend process; preserve its session",
                std::io::Error::from_raw_os_error(error.unwrap_or(libc::EIO)),
            ));
        }
        if length != size_of::<libc::kinfo_proc2>() {
            return Err(AppError::new(
                "Unexpected NetBSD process identity size; preserve its session.",
            ));
        }
        let pointer = info.as_ptr();
        // Each projection stays a raw pointer; it does not create a reference
        // to potentially uninitialized padding or unrelated ABI fields.
        let observed_pid = unsafe { addr_of!((*pointer).p_pid).read() };
        let status = unsafe { addr_of!((*pointer).p_stat).read() };
        if observed_pid != native_pid {
            return Err(AppError::new(
                "Native backend process identity changed unexpectedly.",
            ));
        }
        // Zombies have no valid user area or birth fields on NetBSD.
        if status as i32 == libc::LSZOMB as i32 {
            return Ok(None);
        }
        let valid = unsafe { addr_of!((*pointer).p_uvalid).read() };
        let seconds = unsafe { addr_of!((*pointer).p_ustart_sec).read() };
        let micros = unsafe { addr_of!((*pointer).p_ustart_usec).read() };
        if valid == 0 || seconds == 0 || micros >= 1_000_000 {
            return Err(AppError::new(
                "Incomplete NetBSD process birth identity; preserve its session.",
            ));
        }
        return Ok(Some(Process {
            pid,
            birth: format!("{seconds}:{micros}"),
        }));
    }
    unreachable!("bounded process query returns on its final attempt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_identity_distinguishes_live_zombie_and_reaped_children() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let first = snapshot(std::process::id()).unwrap().unwrap();
        assert_eq!(first.birth, snapshot(first.pid).unwrap().unwrap().birth);
        let mut child = Command::new("/bin/sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let birth =
            super::super::owned_process_identity(&child).expect("live child birth identity");
        assert_eq!(birth.pid, pid);
        child.kill().unwrap();
        let start = Instant::now();
        loop {
            // A dying task can temporarily lose its user area before SZOMB.
            // That must remain an error, rather than an absence claim.
            match snapshot(pid) {
                Ok(None) => break,
                observation => assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "child did not become a zombie: {observation:?}"
                ),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            unsafe { libc::kill(pid as i32, 0) },
            0,
            "unreaped zombie still exists"
        );
        child.wait().unwrap();
        assert!(snapshot(pid).unwrap().is_none());
        assert!(snapshot(0).is_err());
    }
}
