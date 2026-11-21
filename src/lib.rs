use std::env;
use std::ffi::{c_char, CStr, CString};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};

type Cancelled = extern "C" fn() -> bool;

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
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        window: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

fn video_id_is_valid(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn playlist_id_is_valid(id: &str) -> bool {
    (2..=150).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn playlist_url(id: &str) -> String {
    format!("https://www.youtube.com/playlist?list={id}")
}

fn media_url_is_valid(url: &str) -> bool {
    if url.contains('#')
        || url
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return false;
    }
    let Some((host, _)) = url
        .strip_prefix("https://")
        .and_then(|url| url.split_once('/'))
    else {
        return false;
    };
    let host_chars_valid = host
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.');
    host_chars_valid && (host == "googlevideo.com" || host.ends_with(".googlevideo.com"))
}

const HEIGHTS: [u16; 12] = [
    4320, 2160, 1440, 1080, 960, 720, 540, 480, 360, 270, 240, 144,
];

fn exact_video(height: u16) -> String {
    format!("bv[protocol=https][vcodec^=avc1][height={height}]+ba[protocol=https][ext=m4a]/bv[protocol=https][height={height}]+ba[protocol=https]/b[protocol=https][height={height}]/b[protocol^=m3u8][height={height}]")
}

fn resolve(url: &str, mode: &str, cancelled: Cancelled) -> Result<String, &'static str> {
    let video = "b[protocol=https][ext=mp4]/b[protocol=https]/b[protocol^=m3u8]";
    let audio = format!("ba[protocol^=m3u8]/ba[protocol=https]/{video}");
    let hd = format!(
        "bv[protocol=https][vcodec^=avc1][height>=720]+ba[protocol=https][ext=m4a]/bv[protocol=https][height>=720]+ba[protocol=https]/{video}"
    );
    let listing = mode == "qualities";
    let formats = match mode {
        "qualities" => HEIGHTS
            .iter()
            .map(|&height| format!("({})", exact_video(height)))
            .collect::<Vec<_>>()
            .join(","),
        "video" => hd,
        "compatible" => video.to_owned(),
        "audio" => audio,
        _ => {
            let height = mode
                .parse::<u16>()
                .ok()
                .filter(|height| HEIGHTS.contains(height))
                .ok_or("Invalid playback quality.")?;
            exact_video(height)
        }
    };

    let mut args = vec!["--no-playlist", "--format", &formats];
    if listing {
        args.extend(["--print", "%(height)s"]);
    } else {
        args.push("--get-url");
    }
    let output = run_yt_dlp(url, &args, cancelled)?;
    if listing {
        let heights: Vec<_> = HEIGHTS
            .iter()
            .filter(|height| {
                output
                    .lines()
                    .any(|line| line.parse::<u16>().ok().as_ref() == Some(*height))
            })
            .map(u16::to_string)
            .collect();
        return if heights.is_empty() {
            Err("No selectable resolutions. Try Compatible video or Listen.")
        } else {
            Ok(heights.join("\n"))
        };
    }
    let urls: Vec<_> = output.lines().collect();
    if (urls.len() == 1 || (mode != "audio" && mode != "compatible" && urls.len() == 2))
        && urls.iter().all(|url| media_url_is_valid(url))
    {
        Ok(urls.join("\n"))
    } else {
        Err("YouTube did not return a supported VLC stream.")
    }
}

fn run_yt_dlp(url: &str, args: &[&str], cancelled: Cancelled) -> Result<String, &'static str> {
    let path = env::var_os("PATH").unwrap_or_default();
    let executable = env::split_paths(&path)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("yt-dlp.exe"))
        .find(|path| path.is_file())
        .ok_or("Install yt-dlp.exe and restart VLC so it sees your PATH.")?;

    let mut command = Command::new(executable);
    command
        .args([
            "--ignore-config",
            "--no-plugin-dirs",
            "--no-cookies",
            "--no-cookies-from-browser",
            "--no-cache-dir",
            "--no-mark-watched",
            "--socket-timeout",
            "5",
            "--retries",
            "0",
            "--extractor-retries",
            "0",
            "--no-warnings",
        ])
        .args(args)
        .stdin(Stdio::null());
    command.args(["--", url]);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    // VLC's extension watchdog runs after ten seconds.
    let output = bounded_output(&mut command, Duration::from_secs(8), cancelled)?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        return Err(
            if error.contains("sign in")
                || error.contains("login")
                || error.contains("private video")
            {
                "YouTube requires browser playback for this video."
            } else if error.contains("playlist does not exist") {
                "YouTube did not return that playlist. Check the link or open it in your browser."
            } else if error.contains("unable to download") || error.contains("timed out") {
                "Could not reach YouTube. Check your connection or open in browser."
            } else if error.contains("requested format is not available") {
                "That quality is unavailable. Reload qualities or try Compatible video."
            } else {
                "No playable stream. Check yt-dlp and Deno or open in browser."
            },
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

fn playlist_entries(id: &str, cancelled: Cancelled) -> Result<String, &'static str> {
    let output = run_yt_dlp(
        &playlist_url(id),
        &[
            "--yes-playlist",
            "--flat-playlist",
            "--encoding",
            "utf-8",
            "--print",
            "%(id)s\t%(title).160S",
        ],
        cancelled,
    )?;
    let entries: Vec<_> = output
        .lines()
        .filter(|line| {
            line.split_once('\t')
                .is_some_and(|(id, _)| video_id_is_valid(id))
        })
        .collect();
    if entries.is_empty() {
        Err("No public tracks found. Try opening the playlist in your browser.")
    } else {
        Ok(entries.join("\n"))
    }
}

#[cfg(windows)]
fn open_browser(url: &str) -> Result<(), &'static str> {
    let url: Vec<u16> = url.encode_utf16().chain([0]).collect();
    let null = std::ptr::null();
    // Windows uses the default verb for this HTTPS URL. The buffer lives through the call.
    let result = unsafe { ShellExecuteW(0, null, url.as_ptr(), null, null, 1) };
    (result > 32)
        .then_some(())
        .ok_or("Could not open your default browser.")
}

#[cfg(not(windows))]
fn open_browser(_: &str) -> Result<(), &'static str> {
    Err("This bridge requires Windows.")
}

fn run(args: &[String], cancelled: Cancelled) -> Result<String, &'static str> {
    match args {
        [action] if action == "ping" => Ok("YouTube Bridge native module ready.".to_owned()),
        [action, id] if action == "playlist" && playlist_id_is_valid(id) => {
            playlist_entries(id, cancelled)
        }
        [action, id] if action == "playlist-browser" && playlist_id_is_valid(id) => {
            open_browser(&playlist_url(id))?;
            Ok("Sign in on YouTube in your browser if needed.".to_owned())
        }
        [action, id] if action == "qualities" && video_id_is_valid(id) => resolve(
            &format!("https://www.youtube.com/watch?v={id}"),
            "qualities",
            cancelled,
        ),
        [action, id, mode] if action == "play" && video_id_is_valid(id) => resolve(
            &format!("https://www.youtube.com/watch?v={id}"),
            mode,
            cancelled,
        ),
        [action, id] if action == "browser" && video_id_is_valid(id) => {
            open_browser(&format!("https://www.youtube.com/watch?v={id}"))?;
            Ok("Sign in on YouTube in your browser if needed.".to_owned())
        }
        [action] if action == "browser" => {
            open_browser("https://www.youtube.com/")?;
            Ok("Sign in on YouTube in your browser if needed.".to_owned())
        }
        _ => Err("Enter a valid YouTube video link or ID."),
    }
}

// The C shim owns the input string and releases the returned string with bridge_free.
#[no_mangle]
pub unsafe extern "C" fn bridge_run(
    request: *const c_char,
    succeeded: *mut bool,
    cancelled: Cancelled,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let request = CStr::from_ptr(request)
            .to_str()
            .map_err(|_| "Invalid bridge request.")?;
        run(
            &request.split('/').map(str::to_owned).collect::<Vec<_>>(),
            cancelled,
        )
    })
    .unwrap_or(Err("The native bridge could not complete the request."));
    *succeeded = result.is_ok();
    let output = match result {
        Ok(output) => output,
        Err(message) => message.to_owned(),
    };
    CString::new(output.replace('\0', ""))
        .expect("NUL bytes removed")
        .into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn bridge_free(output: *mut c_char) {
    drop(CString::from_raw(output));
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
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
