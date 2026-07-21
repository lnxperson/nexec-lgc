#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

mod vga;
mod keyboard;
mod disk;
mod menu;
mod config;
mod detect;
mod boot_loader;
mod util;
mod allocator;
mod ports;
mod mem;

#[unsafe(no_mangle)]
pub extern "C" fn boot_main() -> ! {
    allocator::init();
    vga::init();
    keyboard::init();
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str("nexec-lgc booting...\r\n");

    let cfg = load_config();
    let detected = if !cfg.no_scan { detect::scan_partition() } else { alloc::vec::Vec::new() };

    let mut menu_ui = menu::Menu::new(&cfg, detected);

    loop {
        vga::clear();
        match menu_ui.run() {
            menu::MenuResult::Boot(entry) => {
                vga::clear();
                vga::set_color(vga::Color::White, vga::Color::Black);
                vga::write_str(&alloc::format!("Booting {}...\r\n", entry.title));
                decrement_boot_counter(&entry);
                backup_entries();
                if !boot_loader::boot_entry(&entry) {
                    menu::show_status(&[
                        ("Boot failed. Press any key for options...", vga::Color::Yellow),
                    ]);
                    keyboard::read_key_blocking();
                }
            }
            menu::MenuResult::Manual => {
                vga::clear();
                if let Some(entry) = menu::browse_files() {
                    vga::write_str(&alloc::format!("Booting {}...\r\n", entry.title));
                    if !boot_loader::boot_entry(&entry) {
                        menu::show_status(&[
                            ("Boot failed.", vga::Color::Yellow),
                            ("Press any key to continue...", vga::Color::DarkGray),
                        ]);
                        keyboard::read_key_blocking();
                    }
                }
            }
            menu::MenuResult::Shutdown => {
                menu::show_status(&[
                    ("Shutdown not supported in this version.", vga::Color::Yellow),
                    ("Press any key to reboot...", vga::Color::DarkGray),
                ]);
                keyboard::read_key_blocking();
                boot_loader::reset_system();
            }
            menu::MenuResult::RestoreBackup => {
                menu::show_status(&[
                    ("Restoring backup...", vga::Color::DarkGray),
                ]);
                if restore_entries() {
                    menu::show_status(&[
                        ("Backup restored!", vga::Color::LightBlue),
                        ("Press any key to reboot...", vga::Color::DarkGray),
                    ]);
                } else {
                    menu::show_status(&[
                        ("No backup found.", vga::Color::Yellow),
                        ("Press any key to continue...", vga::Color::DarkGray),
                    ]);
                }
                keyboard::read_key_blocking();
                boot_loader::reset_system();
            }
        }
    }
}

fn load_config() -> config::Config {
    let mut cfg = config::Config {
        default: None,
        timeout: 5,
        no_scan: false,
        order: None,
        entries: alloc::vec::Vec::new(),
        keybinds: config::Keybinds::default(),
    };

    let mut disk = match disk::Disk::open() {
        Some(d) => d,
        None => return cfg,
    };

    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return cfg,
    };

    let mut fs = match disk::Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return cfg,
    };

    if let Some(data) = fs.read_file("\\nexec.conf") {
        if let Ok(parsed) = config::Config::parse(&data) {
            if cfg.default.is_none() { cfg.default = parsed.default; }
            cfg.timeout = parsed.timeout;
            if cfg.order.is_none() { cfg.order = parsed.order; }
            if parsed.no_scan { cfg.no_scan = true; }
            cfg.keybinds = parsed.keybinds;
            for e in parsed.entries {
                if !cfg.entries.iter().any(|x| x.name == e.name) {
                    cfg.entries.push(e);
                }
            }
        }
    }

    if let Some(entries) = fs.read_dir("\\EFI\\nexec\\entries") {
        for (name_str, data) in entries {
            let raw_name = name_str.trim_end_matches(".conf").trim_end_matches(".CONF");
            let entry_name = raw_name.split('+').next().unwrap_or(raw_name);
            if let Ok(mut parsed) = config::Config::parse_entry_file(entry_name, &data) {
                parsed.boot_counter = parse_counter(raw_name);
                parsed.source_path = Some(alloc::format!("\\EFI\\nexec\\entries\\{}", name_str));
                cfg.entries.retain(|e| e.name != parsed.name);
                cfg.entries.push(parsed);
            }
        }
    }

    cfg
}

fn parse_counter(raw_name: &str) -> Option<u32> {
    if let Some(plus) = raw_name.rfind('+') {
        let suffix = &raw_name[plus + 1..];
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return suffix.parse().ok();
        }
    }
    None
}

fn decrement_boot_counter(entry: &config::Entry) {
    let source = match entry.source_path.as_ref() {
        Some(s) => s,
        None => return,
    };
    let counter = match entry.boot_counter {
        Some(c) if c > 0 => c,
        _ => return,
    };

    let dot_conf = source.len() - 5;
    let name_part = &source[..dot_conf];
    let plus = match name_part.rfind('+') {
        Some(p) => p,
        None => return,
    };
    let base = &name_part[..plus];

    let new_name = if counter == 1 {
        alloc::format!("{}.conf", base)
    } else {
        alloc::format!("{}+{}.conf", base, counter - 1)
    };

    let mut disk = match disk::Disk::open() {
        Some(d) => d,
        None => return,
    };
    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return,
    };
    let mut fs = match disk::Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return,
    };

    let normalized_old = util::normalize_path(source);
    if let Some(data) = fs.read_file(&normalized_old) {
        let normalized_new = util::normalize_path(&new_name);
        let _ = fs.write_file(&normalized_new, &data);
        let _ = fs.delete_file(&normalized_old);
    }
}

fn ensure_backup_dir(fs: &mut disk::Fat32) {
    for p in &["\\EFI\\nexec\\backup", "\\EFI\\nexec\\backup\\entries"] {
        let _ = fs.create_dir(p);
    }
}

fn backup_entries() {
    let mut disk = match disk::Disk::open() {
        Some(d) => d,
        None => return,
    };
    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return,
    };
    let mut fs = match disk::Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return,
    };

    ensure_backup_dir(&mut fs);

    if let Some(entries) = fs.read_dir("\\EFI\\nexec\\entries") {
        for (name, data) in entries {
            if name.ends_with(".conf") || name.ends_with(".CONF") {
                let dst = alloc::format!("\\EFI\\nexec\\backup\\entries\\{}", name);
                let _ = fs.write_file(&dst, &data);
            }
        }
    }
}

fn restore_entries() -> bool {
    let mut disk = match disk::Disk::open() {
        Some(d) => d,
        None => return false,
    };
    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return false,
    };
    let mut fs = match disk::Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return false,
    };

    let mut restored = false;
    if let Some(entries) = fs.read_dir("\\EFI\\nexec\\backup\\entries") {
        for (name, data) in entries {
            if name.ends_with(".conf") || name.ends_with(".CONF") {
                let dst = alloc::format!("\\EFI\\nexec\\entries\\{}", name);
                let _ = fs.write_file(&dst, &data);
                restored = true;
            }
        }
    }
    restored
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    vga::clear();
    vga::set_color(vga::Color::Red, vga::Color::Black);
    vga::write_str("PANIC");
    vga::write_str("\r\n");
    loop {}
}
