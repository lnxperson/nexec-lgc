use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const YELLOW: &str = "\x1b[33m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const DIM: &str = "\x1b[2m";

const BOOTLOADER_DIR: &str = "bootloader";
const BIOS_FILENAME: &str = "nexec-lgc-bios";
const INSTALL_DIR: &str = "/EFI/nexec"; // same paths for consistency
const CLI_INSTALL_PATH: &str = "/usr/bin/nexec-lgc";
const BOOT_CANDIDATES: &[&str] = &["/boot", "/", "/boot/efi"];
const SYS_MOUNTS: &str = "/proc/mounts";
const RELEASE_URL: &str = "https://github.com/person134/nexec-legacy/releases/latest/download";

macro_rules! cprintln {
    ($color:expr, $($arg:tt)*) => { println!("{}{}{}", $color, format_args!($($arg)*), RESET) };
}

pub struct InstallArgs {
    pub boot_path: Option<String>,
    pub disk: Option<String>,
    pub bios_path: Option<String>,
    pub no_build: bool,
    pub no_config: bool,
}

pub fn install(args: InstallArgs) {
    let InstallArgs { boot_path, disk, bios_path, no_build, no_config } = args;

    // 1. Build or locate the bootloader binary
    let bios_binary = if let Some(path) = &bios_path {
        path.clone()
    } else if no_build {
        eprintln!("error: --bios <path> required when --no-build is set");
        std::process::exit(1);
    } else {
        match std::env::current_exe() {
            Ok(exe_path) => {
                let sibling = exe_path.parent().unwrap().join(BIOS_FILENAME);
                if sibling.exists() {
                    cprintln!(GREEN, "Using prebuilt BIOS binary: {}", sibling.display());
                    sibling.to_string_lossy().to_string()
                } else if let Some(downloaded) = download_bios_from_release() {
                    cprintln!(GREEN, "Using prebuilt BIOS binary: {}", downloaded.display());
                    downloaded.to_string_lossy().to_string()
                } else {
                    match build_bootloader() {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("\x1b[31merror:{} {}", RESET, e);
                            eprintln!("  Install the x86_64-unknown-none target:");
                            eprintln!("    rustup target add x86_64-unknown-none");
                            eprintln!("  Or provide a prebuilt binary:");
                            eprintln!("    sudo nexec-lgc install --bios /path/to/nexec-lgc-bios");
                            std::process::exit(1);
                        }
                    }
                }
            }
            Err(_) => {
                match build_bootloader() {
                    Ok(p) => p,
                    Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
                }
            }
        }
    };

    if !Path::new(&bios_binary).exists() {
        eprintln!("\x1b[31merror:{} BIOS binary not found at {}", RESET, bios_binary);
        std::process::exit(1);
    }

    // 2. Detect boot partition
    let boot = boot_path.unwrap_or_else(|| detect_boot().unwrap_or_else(|| {
        eprintln!("\x1b[31merror:{} could not detect boot partition. Specify with --boot", RESET);
        std::process::exit(1);
    }));

    if !Path::new(&boot).is_dir() {
        eprintln!("\x1b[31merror:{} boot path '{}' is not a directory", RESET, boot);
        std::process::exit(1);
    }

    // 3. Copy bootloader binary to the boot partition
    let install_dir = format!("{}{}", boot.trim_end_matches('/'), INSTALL_DIR);
    let install_path = format!("{}/{}", install_dir, BIOS_FILENAME);

    cprintln!(CYAN, "Installing to: {}", install_path);
    std::fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("\x1b[31merror:{} failed to create {}: {}", RESET, install_dir, e);
        std::process::exit(1);
    });
    std::fs::copy(&bios_binary, &install_path).unwrap_or_else(|e| {
        eprintln!("\x1b[31merror:{} failed to copy {}: {}", RESET, bios_binary, e);
        std::process::exit(1);
    });
    cprintln!(GREEN, "  Copied {} -> {}", bios_binary, install_path);

    // 4. Install CLI to system path
    let self_path = std::env::current_exe().unwrap_or_else(|e| {
        cprintln!(YELLOW, "warning: could not determine binary path: {}", e);
        std::process::exit(1);
    });
    let _ = std::fs::remove_file(CLI_INSTALL_PATH);
    std::fs::copy(&self_path, CLI_INSTALL_PATH).unwrap_or_else(|e| {
        eprintln!("\x1b[31merror:{} failed to copy to {}: {} (try running as root)", RESET, CLI_INSTALL_PATH, e);
        std::process::exit(1);
    });
    cprintln!(GREEN, "  Installed CLI to {}", CLI_INSTALL_PATH);

    // 5. Write auto-generated config
    if !no_config {
        let (main_conf, entry_files) = crate::config::generate_detected_config(&boot);

        cprintln!(BOLD_CYAN, "Detected:");
        for (name, content) in &entry_files {
            let title = content.lines()
                .find_map(|l| l.strip_prefix("title = "))
                .unwrap_or(name);
            let efi = content.lines()
                .find_map(|l| l.strip_prefix("efi = "))
                .unwrap_or("");
            cprintln!(BOLD_CYAN, "  Entry: {} ({})", title, name);
            cprintln!(DIM, "    efi: {}", efi);
            if let Some(opts) = content.lines().find_map(|l| l.strip_prefix("options = ")) {
                cprintln!(DIM, "    options: {}", opts);
            }
        }

        let main_conf_path = format!("{}/nexec.conf", install_dir);
        std::fs::write(&main_conf_path, &main_conf).unwrap_or_else(|e| {
            cprintln!(YELLOW, "warning: could not create main config: {}", e);
        });
        cprintln!(GREEN, "  Wrote main config: {}", main_conf_path);

        let entries_dir = format!("{}/entries", install_dir);
        std::fs::create_dir_all(&entries_dir).unwrap_or_else(|e| {
            cprintln!(YELLOW, "warning: could not create entries directory: {}", e);
        });
        for (name, content) in &entry_files {
            let entry_path = format!("{}/{}.conf", entries_dir, name);
            std::fs::write(&entry_path, content).unwrap_or_else(|e| {
                cprintln!(YELLOW, "warning: could not write {}: {}", entry_path, e);
            });
            cprintln!(DIM, "  Wrote entry: {}.conf", name);
        }
        cprintln!(YELLOW, "  Edit entries in {}/ to customize.", entries_dir);
    }

    // 6. Write bootloader to MBR if disk specified
    if let Some(disk_device) = disk {
        write_mbr(&disk_device, &bios_binary);
    } else {
        cprintln!(YELLOW, "  Note: use --disk /dev/sdX to write the bootloader to the MBR.");
        cprintln!(YELLOW, "  Without this, configure your existing bootloader to chainload nexec-lgc.");
    }

    println!();
    cprintln!(BOLD_GREEN, "nexec-lgc installed successfully!");
    cprintln!(YELLOW, "Configure your bootloader to load {}.", install_path);
}

fn download_bios_from_release() -> Option<PathBuf> {
    let tmp = std::env::temp_dir().join("nexec_lgc_download");
    let _ = std::fs::create_dir_all(&tmp);
    let path = tmp.join(BIOS_FILENAME);

    cprintln!(CYAN, "Downloading prebuilt BIOS binary from releases...");
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&path)
        .arg(&format!("{}/{}", RELEASE_URL, BIOS_FILENAME))
        .status()
        .ok()?;

    if status.success() { Some(path) } else { None }
}

fn build_bootloader() -> Result<String, String> {
    let project_root = std::env::current_dir().map_err(|e| e.to_string())?;
    let bootloader_dir = project_root.join(BOOTLOADER_DIR);

    let build_dir = if bootloader_dir.exists() {
        bootloader_dir
    } else {
        let alt = project_root.parent().map(|p| p.join(BOOTLOADER_DIR));
        if let Some(ref alt) = alt { if alt.exists() { alt.clone() } else { return Err("cannot find bootloader/ directory".to_string()); } }
        else { return Err("cannot find bootloader/ directory".to_string()); }
    };

    cprintln!(CYAN, "Building bootloader (x86_64-unknown-none)...");

    let status = Command::new("cargo")
        .args(["build", "--target", "x86_64-unknown-none", "--release"])
        .current_dir(&build_dir)
        .status()
        .map_err(|e| format!("failed to run cargo: {}", e))?;

    if !status.success() {
        return Err("cargo build failed".to_string());
    }

    let binary_path = build_dir
        .join("target")
        .join("x86_64-unknown-none")
        .join("release")
        .join("nexec-lgc-bios");

    Ok(binary_path.to_string_lossy().to_string())
}

fn write_mbr(disk_device: &str, bootloader_path: &str) {
    cprintln!(CYAN, "Writing bootloader to MBR of {}", disk_device);
    let status = Command::new("dd")
        .args(["if=".to_string() + bootloader_path, "of=".to_string() + disk_device, "bs=512".to_string(), "count=1".to_string(), "conv=notrunc".to_string()])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("\x1b[31merror:{} failed to run dd: {}", RESET, e);
            std::process::exit(1);
        });
    if status.success() {
        cprintln!(GREEN, "  MBR written successfully to {}", disk_device);
    } else {
        cprintln!(YELLOW, "  Failed to write MBR to {}", disk_device);
    }
}

pub(crate) fn detect_boot() -> Option<String> {
    for candidate in BOOT_CANDIDATES {
        if Path::new(candidate).is_dir() {
            if let Ok(mounts) = std::fs::read_to_string(SYS_MOUNTS) {
                for line in mounts.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 && parts[1] == *candidate {
                        return Some(candidate.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn status() {
    cprintln!(BOLD_CYAN, "nexec-lgc status");
    cprintln!(DIM, "----------------");

    let boot = detect_boot();
    match boot {
        Some(ref path) => {
            let installed = format!("{}{}/{}", path.trim_end_matches('/'), INSTALL_DIR, BIOS_FILENAME);
            if Path::new(&installed).exists() {
                cprintln!(GREEN, "  Installed: yes ({})", installed);
            } else {
                cprintln!(YELLOW, "  Installed: no");
                cprintln!(DIM, "  Boot partition detected at: {}", path);
                cprintln!(DIM, "  Run 'nexec-lgc install' to install.");
            }
        }
        None => {
            cprintln!(YELLOW, "  Boot partition: not detected");
            cprintln!(DIM, "  Run 'nexec-lgc install --boot <path>' to specify manually.");
        }
    }
}

pub fn remove(boot_path: Option<String>, all: bool, remove_self: bool) {
    let boot = boot_path.unwrap_or_else(|| detect_boot().unwrap_or_else(|| {
        eprintln!("\x1b[31merror:{} could not detect boot partition. Specify with --boot", RESET);
        std::process::exit(1);
    }));

    let mut removed_anything = false;
    let install_dir = format!("{}{}", boot.trim_end_matches('/'), INSTALL_DIR);
    let bios_file = format!("{}/{}", install_dir, BIOS_FILENAME);

    if Path::new(&bios_file).exists() {
        std::fs::remove_file(&bios_file).unwrap_or_else(|e| {
            eprintln!("\x1b[31merror:{} failed to remove {}: {}", RESET, bios_file, e);
            std::process::exit(1);
        });
        cprintln!(GREEN, "  Removed: {}", bios_file);
        removed_anything = true;
    }

    if all {
        let config_paths = [
            format!("{}/nexec.conf", boot.trim_end_matches('/')),
            format!("{}/nexec.conf", install_dir),
        ];
        for cp in &config_paths {
            if Path::new(cp).exists() {
                std::fs::remove_file(cp).unwrap_or_else(|e| {
                    cprintln!(YELLOW, "warning: failed to remove {}: {}", cp, e);
                });
                cprintln!(GREEN, "  Removed: {}", cp);
                removed_anything = true;
            }
        }
        let entries_dir = format!("{}/entries", install_dir);
        if Path::new(&entries_dir).is_dir() {
            if let Ok(dir) = std::fs::read_dir(&entries_dir) {
                for entry in dir.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
            let _ = std::fs::remove_dir(&entries_dir);
            cprintln!(GREEN, "  Removed entries directory");
            removed_anything = true;
        }
    }

    if Path::new(&install_dir).exists() {
        let _ = std::fs::remove_dir(&install_dir);
    }

    if remove_self {
        if Path::new(CLI_INSTALL_PATH).exists() {
            std::fs::remove_file(CLI_INSTALL_PATH).unwrap_or_else(|e| {
                cprintln!(YELLOW, "warning: failed to remove {}: {}", CLI_INSTALL_PATH, e);
            });
            cprintln!(GREEN, "  Removed: {}", CLI_INSTALL_PATH);
            removed_anything = true;
        }
    }

    if removed_anything {
        cprintln!(BOLD_GREEN, "nexec-lgc removed successfully.");
    }
}
