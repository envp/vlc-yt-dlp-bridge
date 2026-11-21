[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$build = Join-Path $project 'target'
New-Item -ItemType Directory -Path $build -Force | Out-Null

if (-not $env:VCToolsInstallDir) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'Install the Visual Studio C++ x64 build tools and Windows SDK.' }
    $vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
    $settings = cmd.exe /d /c "call `"$vcvars`" >nul && set"
    if ($LASTEXITCODE -ne 0) { throw 'Could not initialize the Visual Studio build environment.' }
    foreach ($setting in $settings) {
        if ($setting -match '^([^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
        }
    }
}

$version = '3.0.24'
$archive = Join-Path $build "vlc-$version-source.zip"
$headers = Join-Path $build "vlc-$version\include"
if (-not (Test-Path -LiteralPath (Join-Path $headers 'vlc_plugin.h'))) {
    Invoke-WebRequest -Uri "https://codeload.github.com/videolan/vlc/zip/refs/tags/$version" -OutFile $archive
    $expected = '8E8AFC1BC205BDC578C439B2C930F77EC8DA55C6F2F1A6F48AD0D38144AEAB49'
    if ((Get-FileHash -LiteralPath $archive).Hash -ne $expected) {
        throw 'The downloaded VLC source archive does not match the expected checksum.'
    }
    tar.exe -xf $archive -C $build "vlc-$version/include"
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract the VLC headers.' }
}

cargo build --manifest-path (Join-Path $project 'Cargo.toml') --target-dir $build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'The Rust library did not build.' }

$object = Join-Path $build 'bridge.obj'
clang.exe --target=x86_64-pc-windows-msvc -std=gnu11 -O2 -DHAVE_STDINT_H -Wno-deprecated-declarations -Wno-macro-redefined -I $headers -c (Join-Path $PSScriptRoot 'bridge.c') -o $object
if ($LASTEXITCODE -ne 0) { throw 'The VLC module shim did not compile.' }

$imports = Join-Path $build 'vlccore.lib'
lib.exe /nologo /machine:x64 "/def:$(Join-Path $PSScriptRoot 'vlccore.def')" "/out:$imports"
if ($LASTEXITCODE -ne 0) { throw 'Could not build the VLC core import library.' }

$module = Join-Path $build 'release\libyoutube_bridge_plugin.dll'
$rust = Join-Path $build 'release\vlc_youtube_bridge.lib'
link.exe /nologo /dll /machine:x64 "/out:$module" $object $rust $imports msvcrt.lib ucrt.lib vcruntime.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib synchronization.lib shell32.lib advapi32.lib
if ($LASTEXITCODE -ne 0) { throw 'The native VLC module did not link.' }
Write-Host "Built $module"
