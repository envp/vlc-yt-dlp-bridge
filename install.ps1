[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
$buildDirectory = Join-Path $PSScriptRoot 'target'
cargo build --release --locked --manifest-path $manifest --target-dir $buildDirectory
if ($LASTEXITCODE -ne 0) {
    throw 'The Rust bridge did not build.'
}

$extensions = Join-Path $env:APPDATA 'vlc\lua\extensions'
New-Item -ItemType Directory -Force -Path $extensions | Out-Null
Copy-Item -LiteralPath (Join-Path $buildDirectory 'release\vlc-youtube-bridge.exe') -Destination $extensions
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'vlc-youtube-bridge.lua') -Destination $extensions

foreach ($dependency in 'yt-dlp.exe', 'deno.exe') {
    if (-not (Get-Command $dependency -CommandType Application -ErrorAction SilentlyContinue)) {
        Write-Warning "Install $dependency and make sure it is on PATH before starting VLC."
    }
}
Write-Host 'Restart VLC and open View > YouTube Bridge.'
