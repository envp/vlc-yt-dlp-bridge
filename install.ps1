[CmdletBinding()]
param(
    [switch]$Prebuilt
)

$ErrorActionPreference = 'Stop'
$moduleName = 'libyoutube_bridge_plugin.dll'
if ($Prebuilt) {
    $module = Join-Path $PSScriptRoot $moduleName
} else {
    & (Join-Path $PSScriptRoot 'native\build.ps1')
    $module = Join-Path $PSScriptRoot "target\release\$moduleName"
}
if (-not (Test-Path -LiteralPath $module -PathType Leaf)) {
    throw "Could not find $module. Extract the artifact before installing."
}

$profile = Join-Path $env:APPDATA 'vlc'
$pluginRoot = Join-Path $profile 'youtube-bridge-plugins'
$moduleHash = (Get-FileHash -LiteralPath $module).Hash
$plugins = Join-Path $pluginRoot $moduleHash
$extensions = Join-Path $profile 'lua\extensions'
$modules = Join-Path $profile 'lua\modules'
$playlists = Join-Path $profile 'lua\playlist'
New-Item -ItemType Directory -Force -Path $plugins,$extensions,$modules,$playlists | Out-Null
$installedModule = Join-Path $plugins $moduleName
if (-not (Test-Path -LiteralPath $installedModule) -or
    (Get-FileHash -LiteralPath $installedModule).Hash -ne $moduleHash) {
    Copy-Item -LiteralPath $module -Destination $installedModule
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'vlc-youtube-bridge.lua') -Destination $extensions
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'lua\modules\ytbridge-links.lua') -Destination $modules
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'lua\playlist\ytbridge.lua') -Destination $playlists

$existing = [Environment]::GetEnvironmentVariable('VLC_PLUGIN_PATH', 'User')
$paths = @($existing -split ';' | Where-Object {
    $_ -and $_ -ne $pluginRoot -and -not $_.StartsWith("$pluginRoot\", [StringComparison]::OrdinalIgnoreCase)
})
$updated = ($paths + $plugins) -join ';'
if ($updated -ne $existing) {
    [Environment]::SetEnvironmentVariable('VLC_PLUGIN_PATH', $updated, 'User')
}

foreach ($dependency in 'yt-dlp.exe', 'deno.exe') {
    if (-not (Get-Command $dependency -CommandType Application -ErrorAction SilentlyContinue)) {
        Write-Warning "Install $dependency and make sure it is on PATH before starting VLC."
    }
}
Write-Host 'Close VLC fully, reopen it from the Start menu, and choose View > YouTube Bridge.'
