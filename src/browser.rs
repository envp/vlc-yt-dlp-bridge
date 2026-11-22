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

#[cfg(windows)]
pub(crate) fn open_browser(url: &str) -> Result<(), &'static str> {
    let url: Vec<u16> = url.encode_utf16().chain([0]).collect();
    let null = std::ptr::null();
    // Windows uses the default verb for this HTTPS URL. The buffer lives through the call.
    let result = unsafe { ShellExecuteW(0, null, url.as_ptr(), null, null, 1) };
    (result > 32)
        .then_some(())
        .ok_or("Could not open your default browser.")
}

#[cfg(not(windows))]
pub(crate) fn open_browser(_: &str) -> Result<(), &'static str> {
    Err("This bridge requires Windows.")
}
