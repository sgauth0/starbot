use std::fs;
use std::path::PathBuf;

use crate::output::OutputMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuteMode {
    On,
    Minimal,
    Off,
}

impl CuteMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "on" => Some(Self::On),
            "minimal" => Some(Self::Minimal),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

pub fn print_banner(mode: &OutputMode) {
    if mode.json || mode.quiet {
        return;
    }

    if load_cute_mode() != CuteMode::On {
        return;
    }

    let banner = r#"
      ★   *      .       .   *   .   *
   .     *      .    ★     .      .
        ███████╗████████╗ █████╗ ██████╗ ██████╗  ██████╗ ████████╗████████╗
        ██╔════╝╚══██╔══╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗╚══██╔══╝╚══██╔══╝
        ███████╗   ██║   ███████║██████╔╝██████╔╝██║   ██║   ██║      ██║   
        ╚════██║   ██║   ██╔══██║██╔══██╗██╔══██╗██║   ██║   ██║      ██║   
  ★     ███████║   ██║   ██║  ██║██║  ██║██████╔╝╚██████╔╝   ██║      ██║    ★
        ╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝  ╚═════╝    ╚═╝      ╚═╝   
    .    *    .   *      .   *   .   *
"#;
    eprintln!("{}", banner);
}

pub fn cute_config_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".starbott").join("config"))
}

pub fn load_cute_mode() -> CuteMode {
    let Some(path) = cute_config_path() else {
        return CuteMode::On;
    };

    let Ok(raw) = fs::read_to_string(path) else {
        return CuteMode::On;
    };

    for line in raw.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }

        let (key, value) = match l.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };

        if key.trim().eq_ignore_ascii_case("cute") {
            if let Some(mode) = CuteMode::parse(value) {
                return mode;
            }
        }
    }

    CuteMode::On
}
