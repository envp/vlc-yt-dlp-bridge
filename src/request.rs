use crate::browser::open_browser;
use crate::process::Cancelled;
use crate::resolver::{playlist_entries, resolve};
use crate::youtube::{playlist_id_is_valid, playlist_url, video_id_is_valid};

pub(crate) fn run(args: &[String], cancelled: Cancelled) -> Result<String, &'static str> {
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
