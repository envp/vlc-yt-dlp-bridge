use std::env;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

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

fn resolve(url: &str, mode: &str) -> Result<String, &'static str> {
    let video = "b[protocol=https][ext=mp4]/b[protocol=https]/b[protocol^=m3u8]";
    let audio = format!("ba[protocol=https]/{video}");
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
            "--no-playlist",
            "--socket-timeout",
            "5",
            "--retries",
            "0",
            "--extractor-retries",
            "0",
            "--no-warnings",
            "--format",
            &formats,
        ])
        .stdin(Stdio::null());
    if listing {
        command.args(["--print", "%(height)s"]);
    } else {
        command.arg("--get-url");
    }
    command.args(["--", url]);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let output = command.output().map_err(|_| "Could not start yt-dlp.")?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        return Err(
            if error.contains("sign in")
                || error.contains("login")
                || error.contains("private video")
            {
                "YouTube requires browser playback for this video."
            } else if error.contains("unable to download") || error.contains("timed out") {
                "Could not reach YouTube. Check your connection or open in browser."
            } else if error.contains("requested format is not available") {
                "That quality is unavailable. Reload qualities or try Compatible video."
            } else {
                "No playable stream. Check yt-dlp and Deno or open in browser."
            },
        );
    }
    let output = String::from_utf8_lossy(&output.stdout);
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

fn run(args: &[String]) -> Result<String, &'static str> {
    match args {
        [action, id] if action == "qualities" && video_id_is_valid(id) => resolve(
            &format!("https://www.youtube.com/watch?v={id}"),
            "qualities",
        ),
        [action, id, mode] if action == "play" && video_id_is_valid(id) => {
            resolve(&format!("https://www.youtube.com/watch?v={id}"), mode)
        }
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

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    match run(&args) {
        Ok(message) => println!("{message}"),
        Err(message) => {
            println!("{message}");
            std::process::exit(1);
        }
    }
}
