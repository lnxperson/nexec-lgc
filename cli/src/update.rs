use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_URL: &str = "https://github.com/person134/nexec-legacy/releases/latest/download";

pub fn update() {
    println!("nexec-lgc update v{}", VERSION);
    println!("Fetching latest release...");

    let uid = Command::new("id").arg("-u").output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if uid != "0" {
        eprintln!("error: update requires root");
        std::process::exit(1);
    }

    let tmp = std::env::temp_dir().join("nexec_lgc_update");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap_or_else(|e| { eprintln!("error: failed to create temp dir: {}", e); std::process::exit(1); });

    let cli_path = tmp.join("nexec-lgc");
    let bios_path = tmp.join("nexec-lgc-bios");

    println!("  Downloading nexec-lgc...");
    let status = Command::new("curl")
        .args(["-fsSL", "-o"]).arg(&cli_path)
        .arg(&format!("{}/nexec-lgc", RELEASE_URL))
        .status().unwrap_or_else(|e| { eprintln!("error: failed to run curl: {}", e); std::process::exit(1); });
    if !status.success() { eprintln!("error: failed to download nexec-lgc"); std::process::exit(1); }
    let _ = std::fs::set_permissions(&cli_path, std::os::unix::fs::PermissionsExt::from_mode(0o755));

    println!("  Downloading nexec-lgc-bios...");
    let status = Command::new("curl")
        .args(["-fsSL", "-o"]).arg(&bios_path)
        .arg(&format!("{}/nexec-lgc-bios", RELEASE_URL))
        .status().unwrap_or_else(|e| { eprintln!("error: failed to run curl: {}", e); std::process::exit(1); });
    if !status.success() { eprintln!("error: failed to download nexec-lgc-bios"); std::process::exit(1); }

    println!("  Installing to boot partition...");
    let status = Command::new(&cli_path)
        .args(["install", "--no-config", "--no-build", "--bios"])
        .arg(&bios_path)
        .status().unwrap_or_else(|e| { eprintln!("error: failed to run installer: {}", e); std::process::exit(1); });
    if !status.success() { eprintln!("error: installer failed"); std::process::exit(1); }

    println!("  Updating /usr/bin/nexec-lgc...");
    std::fs::copy(&cli_path, "/usr/bin/nexec-lgc").unwrap_or_else(|e| {
        eprintln!("error: failed to copy to /usr/bin: {}", e);
        std::process::exit(1);
    });
    let _ = std::fs::set_permissions("/usr/bin/nexec-lgc", std::os::unix::fs::PermissionsExt::from_mode(0o755));

    let _ = std::fs::remove_dir_all(&tmp);
    println!();
    println!("nexec-lgc updated successfully!");
}
