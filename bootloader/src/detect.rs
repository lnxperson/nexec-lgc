use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::config::Entry;
use crate::disk::{Disk, Fat32};
use crate::util;

/// Scan the boot partition for installed operating systems.
pub fn scan_partition() -> Vec<Entry> {
    let mut disk = match Disk::open() {
        Some(d) => d,
        None => return Vec::new(),
    };

    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut fs = match Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    // Windows (BIOS): bootmgr on active partition
    if let Some(_data) = fs.read_file("\\bootmgr") {
        entries.push(Entry {
            name: "windows".into(),
            title: "Windows".into(),
            efi_path: "\\bootmgr".into(),
            options: None,
            initrd: Vec::new(),
            boot_counter: None,
            source_path: None,
        });
    }

    // UKIs (\EFI\Linux\*.efi)
    if let Some(ukis) = scan_ukis(&mut fs) {
        entries.extend(ukis);
    }

    // Standalone kernels on partition root
    if let Some(kernels) = scan_kernels(&mut fs) {
        entries.extend(kernels);
    }

    entries
}

fn scan_ukis(fs: &mut Fat32) -> Option<Vec<Entry>> {
    let dir_entries = fs.read_dir("\\EFI\\Linux")?;
    let mut entries = Vec::new();

    for (name, _data) in dir_entries {
        if name.ends_with(".efi") || name.ends_with(".EFI") {
            let path = format!("\\EFI\\Linux\\{}", name);
            let display = name.trim_end_matches(".efi").trim_end_matches(".EFI").replace('-', " ");
            let title: String = display
                .split_whitespace()
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            let entry_name = name.to_lowercase().replace(".efi", "");
            entries.push(Entry {
                name: entry_name,
                title,
                efi_path: path,
                options: None,
                initrd: Vec::new(),
                boot_counter: None,
                source_path: None,
            });
        }
    }

    Some(entries)
}

fn scan_kernels(fs: &mut Fat32) -> Option<Vec<Entry>> {
    // Try to read root directory
    let entries = fs.read_dir("\\")?;
    let mut result = Vec::new();

    for (name, _data) in &entries {
        let version = if name == "vmlinuz" {
            Some("")
        } else if let Some(ver) = name.strip_prefix("vmlinuz-") {
            Some(ver)
        } else {
            None
        };

        if let Some(ver) = version {
            let efi_path = format!("\\{}", name);
            let entry_name = if ver.is_empty() {
                "linux".into()
            } else {
                format!("linux-{}", ver.to_lowercase())
            };
            let title = if ver.is_empty() {
                "Linux".into()
            } else {
                format!("Linux {}", ver)
            };

            let initrd = find_initrd(fs, ver);

            result.push(Entry {
                name: entry_name,
                title,
                efi_path,
                options: None,
                initrd,
                boot_counter: None,
                source_path: None,
            });
        }
    }

    Some(result)
}

fn find_initrd(fs: &mut Fat32, version: &str) -> Vec<String> {
    let candidates: Vec<String> = if version.is_empty() {
        Vec::from([
            String::from("\\initramfs-linux.img"),
            String::from("\\initrd.img"),
            String::from("\\initramfs.img"),
        ])
    } else {
        Vec::from([
            format!("\\initramfs-{}.img", version),
            format!("\\initrd.img-{}", version),
            format!("\\initramfs-{}-generic.img", version),
        ])
    };

    for p in &candidates {
        let normalized = util::normalize_path(p);
        if fs.read_file(&normalized).is_some() {
            return vec![p.clone()];
        }
    }
    Vec::new()
}

/// Scan the entire partition for any bootable files (for manual boot browser).
pub fn scan_all_files() -> Vec<Entry> {
    let mut disk = match Disk::open() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut fs = match Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();
    walk_fs(&mut fs, "\\", &mut entries, 0);
    entries
}

fn walk_fs(fs: &mut Fat32, dir_path: &str, entries: &mut Vec<Entry>, depth: usize) {
    if depth > 6 {
        return;
    }
    let dir_entries = match fs.read_dir(dir_path) {
        Some(e) => e,
        None => return,
    };

    for (name, data) in dir_entries {
        // Check if it's a file or directory - try reading contents
        let is_dir = if name.contains('.') && !name.ends_with(".efi") && !name.ends_with(".EFI") {
            false
        } else if name.ends_with(".efi") || name.ends_with(".EFI") {
            false
        } else {
            // Could be a directory or a file without extension
            // Try reading as directory
            if fs.read_dir(&format!("{}\\{}", dir_path, name)).is_some() {
                true
            } else if data.is_empty() && !name.contains('.') {
                true
            } else {
                false
            }
        };

        if is_dir && name != "." && name != ".." {
            let sub_path = format!("{}\\{}", dir_path, name);
            walk_fs(fs, &sub_path, entries, depth + 1);
        } else if name.ends_with(".efi") || name.ends_with(".EFI") {
            let path = format!("{}\\{}", dir_path, name);
            let title = name
                .trim_end_matches(".efi")
                .trim_end_matches(".EFI");
            let title = String::from(title);
            entries.push(Entry {
                name: title.clone(),
                title,
                efi_path: path,
                options: None,
                initrd: Vec::new(),
                boot_counter: None,
                source_path: None,
            });
        }
    }
}
