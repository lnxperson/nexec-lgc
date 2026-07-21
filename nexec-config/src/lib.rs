#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub title: String,
    pub efi_path: String,
    pub options: Option<String>,
    pub initrd: Vec<String>,
    pub boot_counter: Option<u32>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Keybinds {
    pub manual: char,
    pub firmware: char,
    pub reboot: char,
    pub shutdown: char,
    pub backup: char,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            manual: 'm',
            firmware: 'f',
            reboot: 'r',
            shutdown: 's',
            backup: 'b',
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub default: Option<String>,
    pub timeout: u64,
    pub no_scan: bool,
    pub order: Option<Vec<String>>,
    pub entries: Vec<Entry>,
    pub keybinds: Keybinds,
}

impl Config {
    pub fn parse_entry_file(name: &str, data: &[u8]) -> Result<Entry, &'static str> {
        let text = core::str::from_utf8(data).map_err(|_| "entry file not valid UTF-8")?;
        let mut entry = Entry {
            name: name.to_string(),
            title: String::new(),
            efi_path: String::new(),
            options: None,
            initrd: Vec::new(),
            boot_counter: None,
            source_path: None,
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(eq) = trimmed.find('=') {
                let key = trimmed[..eq].trim();
                let value = trimmed[eq + 1..].trim();
                let value = value.trim_matches('"');
                match key {
                    "title" => entry.title = value.to_string(),
                    "efi" => entry.efi_path = value.to_string(),
                    "options" => {
                        entry.options = if value.is_empty() { None } else { Some(value.to_string()) }
                    }
                    "initrd" => {
                        if !value.is_empty() {
                            entry.initrd.push(value.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        if entry.efi_path.is_empty() {
            return Err("entry file has no efi = path");
        }
        Ok(entry)
    }

    pub fn parse_bls_entry(filename: &str, data: &[u8]) -> Result<Entry, &'static str> {
        let text = core::str::from_utf8(data).map_err(|_| "BLS entry not valid UTF-8")?;
        let mut entry = Entry {
            name: filename.to_string(),
            title: String::new(),
            efi_path: String::new(),
            options: None,
            initrd: Vec::new(),
            boot_counter: None,
            source_path: None,
        };

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(eq) = trimmed.find(char::is_whitespace) {
                let key = trimmed[..eq].trim();
                let value = trimmed[eq + 1..].trim();
                let value = value.trim_matches('"');
                match key {
                    "title" => entry.title = value.to_string(),
                    "linux" => entry.efi_path = value.to_string(),
                    "initrd" => {
                        if !value.is_empty() {
                            entry.initrd.push(value.to_string());
                        }
                    }
                    "options" => {
                        entry.options = if value.is_empty() { None } else { Some(value.to_string()) }
                    }
                    "efi" => entry.efi_path = value.to_string(),
                    _ => {}
                }
            }
        }

        if entry.efi_path.is_empty() {
            return Err("BLS entry has no linux= or efi= path");
        }
        Ok(entry)
    }

    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        let text = core::str::from_utf8(data).map_err(|_| "config not valid UTF-8")?;
        let mut config = Config {
            default: None,
            timeout: 5,
            no_scan: false,
            order: None,
            entries: Vec::new(),
            keybinds: Keybinds::default(),
        };
        let mut current: Option<Entry> = None;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') {
                if let Some(entry) = current.take() {
                    config.entries.push(entry);
                }
                let end = trimmed.find(']').ok_or("unclosed section")?;
                let name = trimmed[1..end].trim();
                if name.is_empty() {
                    return Err("empty section name");
                }
                current = Some(Entry {
                    name: name.to_string(),
                    title: String::new(),
                    efi_path: String::new(),
                    options: None,
                    initrd: Vec::new(),
                    boot_counter: None,
                    source_path: None,
                });
                continue;
            }

            if let Some(eq) = trimmed.find('=') {
                let key = trimmed[..eq].trim();
                let value = trimmed[eq + 1..].trim();
                let value = value.trim_matches('"');

                if let Some(ref mut entry) = current {
                    match key {
                        "title" => entry.title = value.to_string(),
                        "efi" => entry.efi_path = value.to_string(),
                        "options" => {
                            entry.options = if value.is_empty() { None } else { Some(value.to_string()) }
                        }
                        "initrd" => {
                            if !value.is_empty() {
                                entry.initrd.push(value.to_string());
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        "default" => config.default = Some(value.to_string()),
                        "timeout" => {
                            config.timeout = value.parse().map_err(|_| "invalid timeout")?
                        }
                        "no_scan" => {
                            config.no_scan = matches!(value.to_lowercase().as_str(), "true" | "yes" | "1");
                        }
                        "scan" => {
                            config.no_scan = !matches!(value.to_lowercase().as_str(), "true" | "yes" | "1");
                        }
                        "order" => {
                            config.order = Some(
                                value.split_whitespace().map(|s| s.to_string()).collect(),
                            );
                        }
                        "key_manual" => {
                            if let Some(c) = value.chars().next() {
                                config.keybinds.manual = c;
                            }
                        }
                        "key_firmware" => {
                            if let Some(c) = value.chars().next() {
                                config.keybinds.firmware = c;
                            }
                        }
                        "key_reboot" => {
                            if let Some(c) = value.chars().next() {
                                config.keybinds.reboot = c;
                            }
                        }
                        "key_shutdown" => {
                            if let Some(c) = value.chars().next() {
                                config.keybinds.shutdown = c;
                            }
                        }
                        "key_backup" => {
                            if let Some(c) = value.chars().next() {
                                config.keybinds.backup = c;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(entry) = current.take() {
            config.entries.push(entry);
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Config, &'static str> {
        Config::parse(s.as_bytes())
    }

    #[test]
    fn full_config() {
        let data = r#"
default = arch
timeout = 3
order = arch windows

[arch]
title = Arch Linux
efi = /vmlinuz-linux
options = root=UUID=123 rw quiet
initrd = /initramfs-linux.img

[windows]
title = Windows
efi = /EFI/Microsoft/Boot/bootmgfw.efi
"#;
        let cfg = parse(data).unwrap();
        assert_eq!(cfg.default.as_deref(), Some("arch"));
        assert_eq!(cfg.timeout, 3);
        assert!(!cfg.no_scan);
        assert_eq!(cfg.order.as_deref().unwrap(), &["arch", "windows"]);
        assert_eq!(cfg.entries.len(), 2);
    }

    #[test]
    fn empty_config() {
        let cfg = parse("").unwrap();
        assert!(cfg.default.is_none());
        assert_eq!(cfg.timeout, 5);
        assert!(!cfg.no_scan);
        assert!(cfg.order.is_none());
        assert!(cfg.entries.is_empty());
    }

    #[test]
    fn keybind_defaults() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg.keybinds.manual, 'm');
        assert_eq!(cfg.keybinds.firmware, 'f');
        assert_eq!(cfg.keybinds.reboot, 'r');
        assert_eq!(cfg.keybinds.shutdown, 's');
        assert_eq!(cfg.keybinds.backup, 'b');
    }

    #[test]
    fn keybind_custom() {
        let data = "key_manual = a\nkey_firmware = z\nkey_reboot = e\nkey_shutdown = r\nkey_backup = t\n";
        let cfg = parse(data).unwrap();
        assert_eq!(cfg.keybinds.manual, 'a');
        assert_eq!(cfg.keybinds.firmware, 'z');
        assert_eq!(cfg.keybinds.reboot, 'e');
        assert_eq!(cfg.keybinds.shutdown, 'r');
        assert_eq!(cfg.keybinds.backup, 't');
    }

    #[test]
    fn parse_entry_file_minimal() {
        let data = "efi = /vmlinuz-linux";
        let e = Config::parse_entry_file("linux", data.as_bytes()).unwrap();
        assert_eq!(e.name, "linux");
        assert_eq!(e.efi_path, "/vmlinuz-linux");
    }
}
