# YouTube Bridge for VLC

A Windows VLC 3 extension for individual YouTube and YouTube Music links. A local Rust helper resolves streams with yt-dlp. The Lua extension sends the chosen stream to the running VLC player.

## Install

Use desktop VLC, the standalone Windows yt-dlp executable, and Deno. Make sure `yt-dlp.exe` and `deno.exe` are on PATH. See [yt-dlp's YouTube runtime setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS).

To install a prebuilt copy, download the Windows artifact from the [Actions tab](https://github.com/envp/vlc-yt-dlp-bridge/actions). Extract the EXE and Lua files to `%APPDATA%\vlc\lua\extensions`, creating the folder if needed.

To build from source, install Rust with Cargo, the MSVC C++ build tools, and a Windows SDK. From PowerShell in this folder, run:

```powershell
.\install.ps1
```

Restart VLC and choose **View → YouTube Bridge**. Paste an individual link or video ID and select **Load qualities**. The extension lists the available resolutions and fills in the highest one. Edit the resolution if desired and click **Play video**. Leave the resolution blank to select it automatically. For higher resolutions, VLC can open separate video and audio streams together. **Compatible video** selects a single combined stream. **Listen** uses an audio stream when available and otherwise asks VLC to play the audio of a combined stream. If a resolved link expires, use the extension to open it again. Full playlists are handled in the browser.

## Google sign-in

**Open YouTube / Sign in** opens the official YouTube site in your default browser. You can use your existing browser session or sign in there. Private videos, account features, and Premium playback stay in the official player. The bridge never reads browser cookies, credentials, or Google tokens. It also ignores yt-dlp user configuration and plugins so they cannot add account access to bridge requests.

VLC playback uses anonymous yt-dlp requests and can break when YouTube changes stream delivery. YouTube restricts automated access in its [Terms of Service](https://www.youtube.com/t/terms). Use the browser option for official playback.
