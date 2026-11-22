use std::{env, process::Command};

use crate::process::{run_hidden, Cancelled};
use crate::youtube::{playlist_url, video_id_is_valid};

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

pub(crate) fn resolve(url: &str, mode: &str, cancelled: Cancelled) -> Result<String, &'static str> {
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
        .args(["--", url]);
    let output = run_hidden(&mut command, cancelled)?;
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
                "That quality is unavailable. Try another format or open in browser."
            } else {
                "No playable stream. Check yt-dlp and Deno or open in browser."
            },
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn playlist_entries(id: &str, cancelled: Cancelled) -> Result<String, &'static str> {
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
