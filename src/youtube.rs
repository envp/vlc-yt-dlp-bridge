pub(crate) fn video_id_is_valid(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(crate) fn playlist_id_is_valid(id: &str) -> bool {
    (2..=150).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(crate) fn playlist_url(id: &str) -> String {
    format!("https://www.youtube.com/playlist?list={id}")
}
