#[cfg(windows)]
mod windows {
    use sha2::{Digest, Sha256};
    use std::error::Error;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::{env, fs};
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    const MODULE: &str = "libyoutube_bridge_plugin.dll";
    const SCRIPTS: [(&str, &str); 3] = [
        (
            "lua/extensions/vlc-youtube-bridge.lua",
            include_str!("../../vlc-youtube-bridge.lua"),
        ),
        (
            "lua/modules/ytbridge-links.lua",
            include_str!("../../lua/modules/ytbridge-links.lua"),
        ),
        (
            "lua/playlist/ytbridge.lua",
            include_str!("../../lua/playlist/ytbridge.lua"),
        ),
    ];

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            window: isize,
            message: u32,
            wparam: usize,
            lparam: isize,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }

    pub fn install() -> Result<(), Box<dyn Error>> {
        let exe = env::current_exe()?;
        let module = exe.with_file_name(MODULE);
        let bytes = fs::read(&module)
            .map_err(|error| format!("Could not read {}: {error}", module.display()))?;
        let profile = env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        let profile = Path::new(&profile).join("vlc");
        let plugin_root = profile.join("youtube-bridge-plugins");
        let plugins = plugin_root.join(format!("{:x}", Sha256::digest(&bytes)));
        let installed = plugins.join(MODULE);
        fs::create_dir_all(&plugins)?;
        if !fs::read(&installed).is_ok_and(|existing| existing == bytes) {
            fs::write(installed, bytes)?;
        }
        for (relative, script) in SCRIPTS {
            let destination = profile.join(relative);
            fs::create_dir_all(destination.parent().expect("Lua script directory"))?;
            fs::write(destination, script)?;
        }

        register_plugin_path(&plugin_root, &plugins)?;
        println!("Restart VLC and open View > YouTube Bridge.");
        Ok(())
    }

    fn register_plugin_path(root: &Path, plugins: &Path) -> Result<(), Box<dyn Error>> {
        let (registry, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey("Environment")?;
        let previous: String = match registry.get_value("VLC_PLUGIN_PATH") {
            Ok(value) => value,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let root = root.to_string_lossy().to_lowercase();
        let prefix = format!("{root}\\");
        let paths = env::split_paths(&previous)
            .filter(|path| {
                let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
                !path.is_empty() && path != root && !path.starts_with(&prefix)
            })
            .chain([plugins.to_owned()]);
        let updated = env::join_paths(paths)?
            .into_string()
            .map_err(|_| "VLC plugin path could not be represented as UTF-8")?;
        if updated != previous {
            registry.set_value("VLC_PLUGIN_PATH", &updated)?;
            let setting: Vec<u16> = "Environment".encode_utf16().chain([0]).collect();
            // Notify Explorer so newly launched VLC instances inherit the updated path.
            unsafe {
                SendMessageTimeoutW(
                    0xffff,
                    0x1a,
                    0,
                    setting.as_ptr() as isize,
                    2,
                    200,
                    std::ptr::null_mut(),
                );
            }
        }
        Ok(())
    }
}

fn main() {
    #[cfg(windows)]
    if let Err(error) = windows::install() {
        eprintln!("Could not install the VLC bridge: {error}");
        std::process::exit(1);
    }

    #[cfg(not(windows))]
    {
        eprintln!("The VLC bridge installer requires Windows.");
        std::process::exit(1);
    }
}
