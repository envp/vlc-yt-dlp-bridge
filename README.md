# YouTube Bridge for VLC

A Windows x64 VLC 3.0.24 plugin for YouTube and YouTube Music. A native VLC module uses Rust to run yt-dlp directly with a hidden child process. The Lua extension provides the controls under VLC's View menu.

## Install

Use desktop VLC, the standalone Windows yt-dlp executable, and Deno. Make sure `yt-dlp.exe` and `deno.exe` are on PATH. See [yt-dlp's YouTube runtime setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS).

To install a prebuilt copy, download and extract the Windows artifact from the [Actions tab](https://github.com/envp/vlc-yt-dlp-bridge/actions). Use the [Microsoft Visual C++ x64 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) if the DLL fails to load. Run `.\install.ps1 -Prebuilt` in the extracted folder. The script registers a plugin directory in your user-level `VLC_PLUGIN_PATH`.

To build from source, install Rust with Cargo, LLVM Clang on PATH, the MSVC C++ x64 build tools, and a Windows SDK. The build downloads and verifies the VLC 3.0.24 source headers. From PowerShell in this folder, run:

```powershell
.\install.ps1
```

Restart VLC and choose **View → YouTube Bridge**. Paste a video, track, or playlist link. Click **Play video** for automatic video quality or **Listen** for audio only. The dialog closes when VLC accepts the request. **Open in browser** opens YouTube in your default browser.

You can also drag a YouTube link from your browser into VLC's main window or paste it into **Media → Open Network Stream**. Direct links use video mode for YouTube and audio mode for YouTube Music. If a resolved link expires, open the original YouTube link again.

## Playlists

To queue a public playlist, use its playlist page link with `/playlist?list=…`. A watch link opens its individual video even when the link includes a playlist ID. VLC expands the playlist and resolves each track when it starts, so a transition can take a few seconds. Use **Open in browser** for playlists that require an account.

## Google sign-in

Use **Open in browser** to use your existing YouTube session or sign in there. Private videos, account features, and Premium playback stay in the official player. The bridge never reads browser cookies, credentials, or Google tokens. It also ignores yt-dlp user configuration and plugins so they cannot add account access to bridge requests.

VLC playback uses anonymous yt-dlp requests and can break when YouTube changes stream delivery. YouTube restricts automated access in its [Terms of Service](https://www.youtube.com/t/terms). Use the browser option for official playback.
