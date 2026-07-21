use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

type DetectedEntry = (String, String, String, Option<String>, Vec<String>);

pub fn init(output: String) {
    let path = Path::new(&output);
    if path.exists() {
        eprintln!("error: {} already exists", output);
        std::process::exit(1);
    }

    let main_content = r#"# nexec-lgc main configuration
# Place this file at /EFI/nexec/nexec.conf on your boot partition.
# Boot entries go in /EFI/nexec/entries/*.conf (one file per entry).
default = arch
timeout = 5
# order = arch windows    # uncomment to set display order by entry name
# no_scan = true          # uncomment to disable auto-detection of OSes
# Keybinds — configure which keys trigger each action.
# Set these to match your keyboard layout.
# key_manual = m
# key_firmware = f
# key_reboot = r
# key_shutdown = s
# key_backup = b
"#;

    let main_path = if output.ends_with('/') || output.ends_with("\\") || path.is_dir() {
        Path::new(&output).join("nexec.conf")
    } else if output.ends_with("nexec.conf") {
        Path::new(&output).to_path_buf()
    } else {
        Path::new(&output).join("nexec.conf")
    };

    let parent = main_path.parent().unwrap_or_else(|| {
        eprintln!("error: invalid output path with no parent directory");
        std::process::exit(1);
    });
    std::fs::create_dir_all(parent).unwrap_or_else(|e| {
        eprintln!("error: failed to create directories: {}", e);
        std::process::exit(1);
    });
    std::fs::write(&main_path, main_content).unwrap_or_else(|e| {
        eprintln!("error: failed to write {}: {}", main_path.display(), e);
        std::process::exit(1);
    });
    println!("Created: {}", main_path.display());

    let entries_dir = main_path.parent().unwrap().join("entries");
    std::fs::create_dir_all(&entries_dir).unwrap_or_else(|e| {
        eprintln!("error: failed to create {}: {}", entries_dir.display(), e);
        std::process::exit(1);
    });

    let arch_entry = r#"title = Arch Linux
efi = /vmlinuz-linux
initrd = /initramfs-linux.img
options = root=UUID=your-root-uuid rw quiet
"#;
    std::fs::write(entries_dir.join("arch.conf"), arch_entry).unwrap_or_else(|e| {
        eprintln!("error: failed to write arch.conf: {}", e);
        std::process::exit(1);
    });
    println!("Created: {}", entries_dir.join("arch.conf").display());

    let win_entry = r#"title = Windows
efi = /bootmgr
"#;
    std::fs::write(entries_dir.join("windows.conf"), win_entry).unwrap_or_else(|e| {
        eprintln!("error: failed to write windows.conf: {}", e);
        std::process::exit(1);
    });
    println!("Created: {}", entries_dir.join("windows.conf").display());

    println!();
    println!("Edit the files and place them on your boot partition:");
    println!("  /EFI/nexec/nexec.conf            (main config)");
    println!("  /EFI/nexec/entries/*.conf        (one file per entry)");
}

fn current_os() -> Option<(String, String)> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    let mut id = None;
    let mut name = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("ID=") {
            id = Some(val.trim_matches('"').to_lowercase());
        }
        if let Some(val) = line.strip_prefix("NAME=") {
            name = Some(val.trim_matches('"').to_string());
        }
    }
    Some((id.unwrap_or_else(|| "linux".into()), name.unwrap_or_else(|| "Linux".into())))
}

fn find_kernel() -> Option<PathBuf> {
    if let Ok(output) = Command::new("uname").arg("-r").output() {
        let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !ver.is_empty() {
            let by_version = format!("/boot/vmlinuz-{}", ver);
            if Path::new(&by_version).exists() {
                return Some(PathBuf::from(by_version));
            }
        }
    }
    if let Ok(dir) = std::fs::read_dir("/boot") {
        let mut candidates: Vec<PathBuf> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.starts_with("vmlinuz")
            })
            .collect();
        candidates.sort();
        candidates.into_iter().last()
    } else {
        None
    }
}

fn find_initrd(kernel: &Path) -> Option<PathBuf> {
    let name = kernel.file_name()?.to_string_lossy();
    let stem = name.strip_prefix("vmlinuz-").unwrap_or("");
    let patterns = if stem.is_empty() {
        vec!["/boot/initramfs-linux.img".into(), "/boot/initrd.img".into(), "/boot/initramfs.img".into()]
    } else {
        vec![format!("/boot/initramfs-{}.img", stem), format!("/boot/initrd.img-{}", stem)]
    };
    for p in &patterns {
        let pb = PathBuf::from(p);
        if pb.exists() { return Some(pb); }
    }
    None
}

fn to_boot_path(abs_path: &Path, boot: &Path) -> Option<String> {
    let boot_canon = boot.canonicalize().ok()?;
    let abs = abs_path.canonicalize().unwrap_or_else(|_| abs_path.to_path_buf());
    let rest = abs.strip_prefix(&boot_canon).ok()
        .or_else(|| abs_path.strip_prefix(&boot_canon).ok())?;
    let components: Vec<_> = rest.components().map(|c| c.as_os_str().to_string_lossy()).collect();
    if components.is_empty() { return None; }
    Some(format!("/{}", components.join("/")))
}

fn resolve_root_device() -> Option<String> {
    let target = Command::new("mountpoint").args(["-d", "/"]).output().ok()
        .and_then(|o| if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (!s.is_empty()).then_some(s)
        } else { None })?;
    let content = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 && fields[2] == target {
            if let Some(dash) = fields.iter().position(|&f| f == "-") {
                if dash + 3 < fields.len() {
                    let source = fields[dash + 2];
                    if source.starts_with("/dev/") && !source.contains("/loop") && !source.contains("/ram") {
                        return Some(source.to_string());
                    }
                }
            }
        }
    }
    None
}

fn detect_root_param() -> Option<String> {
    let device = resolve_root_device()?;
    if let Ok(output) = Command::new("blkid").args(["-s", "UUID", "-o", "value", &device]).output() {
        let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !uuid.is_empty() { return Some(format!("root=UUID={}", uuid)); }
    }
    Some(format!("root={}", device))
}

fn in_live_env(cmdline: &str) -> bool {
    cmdline.split_whitespace().any(|p| p.starts_with("archiso"))
}

fn build_linux_options() -> String {
    let cmdline = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let cmdline = cmdline.trim();
    let has_root = cmdline.split_whitespace().any(|p| p.starts_with("root="));
    if has_root && !in_live_env(cmdline) {
        build_linux_options_from(cmdline)
    } else if let Some(root) = detect_root_param() {
        build_linux_options_from(&root)
    } else {
        build_linux_options_from("")
    }
}

fn build_linux_options_from(cmdline: &str) -> String {
    let mut opts = cmdline.to_string();
    opts = opts.split_whitespace()
        .filter(|p| !p.starts_with("initrd=") && !p.starts_with("archiso"))
        .collect::<Vec<_>>().join(" ");
    if !opts.split_whitespace().any(|p| p == "rootwait") {
        opts = if opts.is_empty() { "rootwait".into() } else { format!("{} rootwait", opts) };
    }
    if !opts.split_whitespace().any(|p| p == "rw") {
        opts = if opts.is_empty() { "rw".into() } else { format!("{} rw", opts) };
    }
    if !opts.split_whitespace().any(|p| p.starts_with("panic=")) {
        opts = format!("{} panic=10", opts);
    }
    opts.split_whitespace().filter(|p| *p != "quiet").collect::<Vec<_>>().join(" ")
}

pub fn generate_detected_config(boot: &str) -> (String, Vec<(String, String)>) {
    let boot_path = Path::new(boot.trim_end_matches('/'));
    let mut entry_files: Vec<DetectedEntry> = Vec::new();

    // Windows
    let win = boot_path.join("bootmgr");
    if win.exists() {
        entry_files.push(("windows".into(), "Windows".into(), "/bootmgr".into(), None, Vec::new()));
    }

    // Detect the running OS
    if let Some((os_id, os_name)) = current_os() {
        let kernel_on_boot = find_kernel().and_then(|kp| to_boot_path(&kp, boot_path));
        let kernel_on_root = scan_boot_root_kernels(boot_path);

        let (efi_path, initrd_list) = if let Some(ref path) = kernel_on_boot {
            let kernel_path = find_kernel().unwrap();
            let mut initrds = Vec::new();
            if let Some(ip) = find_initrd(&kernel_path).and_then(|p| to_boot_path(&p, boot_path)) {
                initrds.push(ip);
            }
            (path.clone(), initrds)
        } else if let Some(path) = kernel_on_root {
            (path, Vec::new())
        } else {
            eprintln!("warning: could not find any kernel on the boot partition.");
            (String::new(), Vec::new())
        };

        if !efi_path.is_empty() {
            let opts = build_linux_options();
            entry_files.push((os_id, os_name, efi_path, Some(opts), initrd_list));
        }
    }

    // UKIs
    let linux_dir = boot_path.join("EFI/Linux");
    if linux_dir.is_dir() {
        if let Ok(dir_entries) = std::fs::read_dir(&linux_dir) {
            for entry in dir_entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy().to_string();
                if name.ends_with(".efi") || name.ends_with(".EFI") {
                    let display = name.trim_end_matches(".efi").trim_end_matches(".EFI").replace('-', " ");
                    let title: String = display.split_whitespace()
                        .map(|w| {
                            let mut chars = w.chars();
                            match chars.next() {
                                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                None => String::new(),
                            }
                        }).collect::<Vec<_>>().join(" ");
                    let entry_name = name.to_lowercase().replace(".efi", "");
                    let efi_path = format!("/EFI/Linux/{}", name);
                    entry_files.push((entry_name, title, efi_path, None, Vec::new()));
                }
            }
        }
    }

    scan_boot_root_kernels_all(boot_path, &mut entry_files);

    let mut seen: Vec<String> = Vec::new();
    entry_files.retain(|(_, _, p, _, _)| {
        if seen.contains(p) { false } else { seen.push(p.clone()); true }
    });

    let mut main_conf = String::new();
    main_conf.push_str("# nexec-lgc main configuration\n");
    main_conf.push_str("# Boot entries are in /EFI/nexec/entries/*.conf\n");
    main_conf.push_str("no_scan = true\n");
    main_conf.push_str("timeout = 5\n");

    if !entry_files.is_empty() {
        let order: Vec<&str> = entry_files.iter().map(|(n, _, _, _, _)| n.as_str()).collect();
        main_conf.push_str(&format!("order = {}\n", order.join(" ")));
    }

    main_conf.push_str("# Keybinds (change these for non-QWERTY layouts)\n");
    main_conf.push_str("key_manual = m\n");
    main_conf.push_str("key_firmware = f\n");
    main_conf.push_str("key_reboot = r\n");
    main_conf.push_str("key_shutdown = s\n");
    main_conf.push_str("key_backup = b\n");

    let entries: Vec<(String, String)> = entry_files.iter()
        .map(|(name, title, efi_path, options, initrd_list)| {
            let mut content = format!("title = {}\n", title);
            content.push_str(&format!("efi = {}\n", efi_path));
            for ird in initrd_list { content.push_str(&format!("initrd = {}\n", ird)); }
            if let Some(opts) = options { content.push_str(&format!("options = {}\n", opts)); }
            (name.clone(), content)
        }).collect();

    (main_conf, entries)
}

fn scan_boot_root_kernels(boot_path: &Path) -> Option<String> {
    if let Ok(dir) = std::fs::read_dir(boot_path.join("")) {
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if name.starts_with("vmlinuz") { return Some(format!("/{}", name)); }
        }
    }
    None
}

fn scan_boot_root_kernels_all(boot_path: &Path, entries: &mut Vec<DetectedEntry>) {
    if let Ok(dir) = std::fs::read_dir(boot_path.join("")) {
        let mut kernels: Vec<String> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("vmlinuz"))
            .collect();
        kernels.sort();
        for name in kernels {
            let efi_path = format!("/{}", name);
            let version = name.strip_prefix("vmlinuz-").unwrap_or("");
            let title = if version.is_empty() { "Linux".into() } else { format!("Linux {}", version) };
            let entry_name = if version.is_empty() { "linux".into() } else { format!("linux-{}", version.to_lowercase()) };
            let opts = build_linux_options();
            entries.push((entry_name, title, efi_path, Some(opts), Vec::new()));
        }
    }
}

pub fn detect(boot_path: Option<String>) {
    let boot = boot_path.unwrap_or_else(|| {
        super::install::detect_boot().unwrap_or_else(|| {
            eprintln!("error: could not detect boot partition. Specify with --boot");
            std::process::exit(1);
        })
    });
    let (main_conf, entry_files) = generate_detected_config(&boot);
    println!("=== Main config (nexec.conf) ===");
    println!("{}", main_conf);
    println!("=== Entry files (entries/*.conf) ===");
    for (name, content) in &entry_files {
        println!("--- {}.conf ---", name);
        println!("{}", content);
    }
}

pub fn set_default(entry: String, boot_path: Option<String>) {
    let boot = boot_path.unwrap_or_else(|| {
        super::install::detect_boot().unwrap_or_else(|| {
            eprintln!("error: could not detect boot partition. Specify with --boot");
            std::process::exit(1);
        })
    });
    let boot = boot.trim_end_matches('/');

    let config_candidates = [
        format!("{}/EFI/nexec/nexec.conf", boot),
        format!("{}/nexec.conf", boot),
    ];

    let config_path = config_candidates.iter().find(|p| Path::new(p).exists());
    let path = match config_path {
        Some(p) => p.clone(),
        None => { eprintln!("error: no nexec.conf found"); std::process::exit(1); }
    };

    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: failed to read {}: {}", path, e);
        std::process::exit(1);
    });

    let mut found = false;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with("default") && trimmed.contains('=') {
            *line = format!("default = {}", entry);
            found = true;
            break;
        }
    }

    if !found {
        let entry_path = format!("{}/EFI/nexec/entries/{}.conf", boot, entry);
        if !Path::new(&entry_path).exists() {
            eprintln!("warning: entry '{}' not found", entry);
        }
        lines.push(String::new());
        lines.push(format!("default = {}", entry));
    }

    let new_content = lines.join("\n");
    std::fs::write(&path, new_content).unwrap_or_else(|e| {
        eprintln!("error: failed to write {}: {}", path, e);
        std::process::exit(1);
    });
    println!("Default entry set to '{}' in {}", entry, path);
}

pub fn edit(boot_path: Option<String>) {
    let boot = boot_path.unwrap_or_else(|| {
        super::install::detect_boot().unwrap_or_else(|| {
            eprintln!("error: could not detect boot partition. Specify with --boot");
            std::process::exit(1);
        })
    });
    let boot = boot.trim_end_matches('/');

    let config_candidates = [
        format!("{}/EFI/nexec/nexec.conf", boot),
        format!("{}/nexec.conf", boot),
    ];

    let path = config_candidates.iter().find(|p| Path::new(p).exists());
    let path = match path {
        Some(p) => p.clone(),
        None => { eprintln!("error: no nexec.conf found"); std::process::exit(1); }
    };

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "nano".to_string());

    println!("Opening config: {}", path);
    let status = Command::new(&editor).arg(&path).status().unwrap_or_else(|e| {
        eprintln!("error: failed to run editor '{}': {}", editor, e);
        std::process::exit(1);
    });
    if !status.success() {
        eprintln!("warning: editor exited with non-zero status");
    }
}
