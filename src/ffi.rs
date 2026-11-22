use std::ffi::{c_char, CStr, CString};

use crate::process::Cancelled;
use crate::request::run;

/// Execute a bridge request and return its UTF-8 response.
/// Release the returned string with `bridge_free`.
///
/// # Safety
/// `request` must be a readable NUL-terminated string and `succeeded` must
/// point to a writable boolean. `cancelled` must be callable until this returns.
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

/// Release a response allocated by `bridge_run`.
///
/// # Safety
/// Pass a pointer returned by `bridge_run` exactly once.
#[no_mangle]
pub unsafe extern "C" fn bridge_free(output: *mut c_char) {
    drop(CString::from_raw(output));
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use std::ptr::NonNull;

    struct Response(NonNull<c_char>);

    impl Response {
        fn text(&self) -> &str {
            unsafe { CStr::from_ptr(self.0.as_ptr()) }
                .to_str()
                .expect("bridge response must be UTF-8")
        }
    }

    impl Drop for Response {
        fn drop(&mut self) {
            unsafe { bridge_free(self.0.as_ptr()) };
        }
    }

    fn request(input: &[u8], initial_status: bool) -> (bool, Response) {
        extern "C" fn unexpected_cancellation_check() -> bool {
            true
        }

        let input = CString::new(input).expect("test input contains no NUL bytes");
        let mut succeeded = initial_status;
        let output = unsafe {
            bridge_run(
                input.as_ptr(),
                &mut succeeded,
                unexpected_cancellation_check,
            )
        };
        (
            succeeded,
            Response(
                NonNull::new(output).expect("bridge must return a response on success and failure"),
            ),
        )
    }

    #[test]
    fn ping_sets_success_and_returns_a_c_string() {
        let (succeeded, response) = request(b"ping", false);
        assert!(succeeded);
        assert_eq!(response.text(), "YouTube Bridge native module ready.");
    }

    #[test]
    fn invalid_requests_clear_success_and_return_an_error() {
        for input in [
            "",
            "unknown",
            "ping/extra",
            "play/short/audio",
            "play/SmMS5H5XChU",
            "qualities/SmMS5H5XCh!",
            "playlist/PL?unexpected",
            "playlist-browser/x",
            "browser/SmMS5H5XCh!",
        ] {
            let (succeeded, response) = request(input.as_bytes(), true);
            assert!(!succeeded, "{input}");
            assert_eq!(
                response.text(),
                "Enter a valid YouTube video link or ID.",
                "{input}"
            );
        }
    }

    #[test]
    fn invalid_utf8_is_rejected_at_the_api_boundary() {
        let (succeeded, response) = request(b"play/\xff/audio", true);
        assert!(!succeeded);
        assert_eq!(response.text(), "Invalid bridge request.");
    }

    #[test]
    fn unsupported_quality_is_rejected_before_resolving() {
        let (succeeded, response) = request(b"play/SmMS5H5XChU/9999", true);
        assert!(!succeeded);
        assert_eq!(response.text(), "Invalid playback quality.");
    }

    #[test]
    fn response_buffers_can_be_released_independently() {
        let (_, first) = request(b"ping", false);
        let (_, second) = request(b"ping/extra", true);
        assert_ne!(first.0, second.0);
        drop(first);
        assert_eq!(second.text(), "Enter a valid YouTube video link or ID.");
    }
}
