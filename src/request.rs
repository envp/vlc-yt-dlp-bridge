use crate::process::Cancelled;
use crate::resolver::{playlist_entries, resolve};
use crate::youtube::{playlist_id_is_valid, video_id_is_valid};

pub(crate) fn run(args: &[String], cancelled: Cancelled) -> Result<String, &'static str> {
    match args {
        [action] if action == "ping" => Ok("YouTube Bridge native module ready.".to_owned()),
        [action, id] if action == "playlist" && playlist_id_is_valid(id) => {
            playlist_entries(id, cancelled)
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
        _ => Err("Enter a valid YouTube video link or ID."),
    }
}
