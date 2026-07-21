use std::path::Path;

pub fn detect(boot_path: Option<String>) {
    let boot = boot_path.unwrap_or_else(|| {
        super::install::detect_boot().unwrap_or_else(|| {
            eprintln!("error: could not detect boot partition. Specify with --boot");
            std::process::exit(1);
        })
    });
    let boot = boot.trim_end_matches('/');

    println!("Boot partition: {}", boot);
    println!();
    println!("Detected entries:");
    println!("-----------------");

    // Windows bootmgr
    let windows_path = format!("{}/bootmgr", boot);
    if Path::new(&windows_path).exists() {
        println!("  Windows Boot Manager");
        println!("    efi: /bootmgr");
    }

    // UKIs
    let linux_dir = format!("{}/EFI/Linux", boot);
    if Path::new(&linux_dir).is_dir() {
        if let Ok(entries) = std::fs::read_dir(&linux_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if name.ends_with(".efi") || name.ends_with(".EFI") {
                        println!("  Linux UKI: {}", name);
                        println!("    efi: /EFI/Linux/{}", name);
                    }
                }
            }
        }
    }

    // Standalone kernels on boot partition root
    if let Ok(dir) = std::fs::read_dir(boot) {
        let mut kernels: Vec<String> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("vmlinuz"))
            .collect();
        kernels.sort();
        for name in kernels {
            let version = name.strip_prefix("vmlinuz-").unwrap_or("(no version)");
            println!("  Linux kernel: {}", name);
            println!("    version: {}", version);
            println!("    efi: /{}", name);
        }
    }

    // Config files
    let config_paths = [
        format!("{}/EFI/nexec/nexec.conf", boot),
        format!("{}/EFI/nexec/entries", boot),
        format!("{}/nexec.conf", boot),
    ];
    for cp in &config_paths {
        if Path::new(cp).exists() {
            println!("  Config: {}", cp);
        }
    }
}
