use std::process::{Command, Output, Stdio};
use std::time::Duration;

#[cfg(windows)]
use std::{
    io::Read,
    os::windows::{io::AsRawHandle, process::CommandExt},
    thread,
    time::Instant,
};

pub(crate) type Cancelled = extern "C" fn() -> bool;

pub(crate) fn run_hidden(
    command: &mut Command,
    cancelled: Cancelled,
) -> Result<Output, &'static str> {
    command.stdin(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    bounded_output(command, Duration::from_secs(8), cancelled)
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn PeekNamedPipe(
        pipe: std::os::windows::io::RawHandle,
        buffer: *mut u8,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        remaining: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
fn bounded_output(
    command: &mut Command,
    timeout: Duration,
    cancelled: Cancelled,
) -> Result<Output, &'static str> {
    const TIMED_OUT: &str = "YouTube lookup timed out. Try again or open it in your browser.";
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Could not start yt-dlp.")?;
    let deadline = Instant::now() + timeout;
    let mut out_pipe = child.stdout.take().expect("piped stdout");
    let mut err_pipe = child.stderr.take().expect("piped stderr");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let result = loop {
        if cancelled() {
            break Err("YouTube lookup cancelled.");
        }
        if Instant::now() >= deadline {
            break Err(TIMED_OUT);
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => break Err("Could not monitor yt-dlp."),
        };
        let out_read = drain_pipe(&mut out_pipe, &mut stdout);
        let err_read = drain_pipe(&mut err_pipe, &mut stderr);
        match (out_read, err_read, status) {
            (Ok(0), Ok(0), Some(status)) => {
                break Ok(Output {
                    status,
                    stdout,
                    stderr,
                })
            }
            (Ok(0), Ok(0), None) => thread::sleep(Duration::from_millis(20)),
            (Ok(_), Ok(_), _) => {}
            _ => break Err("Could not read yt-dlp output."),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[cfg(windows)]
fn drain_pipe(
    pipe: &mut (impl Read + AsRawHandle),
    output: &mut Vec<u8>,
) -> std::io::Result<usize> {
    let mut available = 0;
    // Peek before reading so a slow process or an inherited pipe cannot block VLC.
    let success = unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(109) {
            Ok(0)
        } else {
            Err(error)
        };
    }
    let mut buffer = [0; 8192];
    let amount = (available as usize).min(buffer.len());
    if amount == 0 {
        return Ok(0);
    }
    let read = pipe.read(&mut buffer[..amount])?;
    output.extend_from_slice(&buffer[..read]);
    Ok(read)
}

#[cfg(not(windows))]
fn bounded_output(_: &mut Command, _: Duration, _: Cancelled) -> Result<Output, &'static str> {
    Err("This bridge requires Windows.")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn slow_subprocess_returns_before_it_finishes() {
        let ping = PathBuf::from(env::var_os("SystemRoot").expect("Windows system root"))
            .join("System32")
            .join("ping.exe");
        let mut command = Command::new(ping);
        command.args(["-n", "12", "127.0.0.1"]);
        let start = Instant::now();
        extern "C" fn active() -> bool {
            false
        }
        let error = bounded_output(&mut command, Duration::from_millis(250), active).unwrap_err();
        assert!(error.contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancelled_lookup_terminates_subprocess() {
        let ping = PathBuf::from(env::var_os("SystemRoot").expect("Windows system root"))
            .join("System32")
            .join("ping.exe");
        let mut command = Command::new(ping);
        command.args(["-n", "12", "127.0.0.1"]);
        extern "C" fn stopped() -> bool {
            true
        }
        let start = Instant::now();
        let error = bounded_output(&mut command, Duration::from_secs(8), stopped).unwrap_err();
        assert!(error.contains("cancelled"));
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
