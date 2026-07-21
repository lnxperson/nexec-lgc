use std::path::Path;
use std::process::Command;

fn get_boot(boot: Option<String>) -> String {
    boot.unwrap_or_else(|| {
        super::install::detect_boot().unwrap_or_else(|| {
            eprintln!("error: could not detect boot partition. Specify with --boot");
            std::process::exit(1);
        })
    })
}

fn entries_dir(boot: &str) -> String {
    format!("{}/EFI/nexec/entries", boot.trim_end_matches('/'))
}

pub fn list(boot: Option<String>) {
    let boot = get_boot(boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);
    if !dir_path.is_dir() {
        eprintln!("error: entries directory not found at {}", dir);
        std::process::exit(1);
    }

    println!("Boot entries in {}:", dir);
    let mut files: Vec<_> = std::fs::read_dir(dir_path)
        .unwrap_or_else(|e| { eprintln!("error: failed to read {}: {}", dir, e); std::process::exit(1); })
        .filter_map(|e| e.ok())
        .filter(|e| { let name = e.file_name(); let n = name.to_string_lossy(); n.ends_with(".conf") })
        .collect();
    files.sort_by_key(|e| e.file_name());

    for file in &files {
        let name = file.file_name().to_string_lossy().to_string();
        let content = std::fs::read_to_string(file.path()).unwrap_or_default();
        let title = content.lines().find_map(|l| l.strip_prefix("title = ")).unwrap_or("(no title)");
        let raw = name.trim_end_matches(".conf");
        let tries = if let Some(plus) = raw.rfind('+') {
            let suffix = &raw[plus + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                format!(" [{} tries]", suffix)
            } else { String::new() }
        } else { String::new() };

        println!("  {}{}", name, tries);
        println!("    title: {}", title);
        if let Some(efi) = content.lines().find_map(|l| l.strip_prefix("efi = ")) {
            println!("    efi: {}", efi);
        }
    }
}

fn backup_entries(boot: &str) {
    let boot = boot.trim_end_matches('/');
    let entries_dir = format!("{}/EFI/nexec/entries", boot);
    let backup_dir = format!("{}/EFI/nexec/backup/entries", boot);
    let entries_path = Path::new(&entries_dir);
    if !entries_path.is_dir() { return; }
    let backup_path = Path::new(&backup_dir);
    let _ = std::fs::create_dir_all(backup_path);
    if let Ok(dir) = std::fs::read_dir(backup_path) {
        for entry in dir.flatten() { let _ = std::fs::remove_file(entry.path()); }
    }
    if let Ok(dir) = std::fs::read_dir(entries_path) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".conf") {
                let _ = std::fs::copy(&entry.path(), backup_path.join(&name));
            }
        }
    }
}

pub fn add(name: String, boot: Option<String>, efi: String, title: Option<String>, options: Option<String>, initrd: Option<String>, tries: Option<u32>) {
    let boot = get_boot(boot);
    backup_entries(&boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);
    std::fs::create_dir_all(dir_path).unwrap_or_else(|e| { eprintln!("error: failed to create {}: {}", dir, e); std::process::exit(1); });

    let filename = if let Some(t) = tries { format!("{}+{}.conf", name, t) } else { format!("{}.conf", name) };
    let entry_path = dir_path.join(&filename);
    if entry_path.exists() { eprintln!("error: entry already exists at {}", entry_path.display()); std::process::exit(1); }

    let t = title.unwrap_or_else(|| name.clone());
    let mut content = format!("title = {}\n", t);
    content.push_str(&format!("efi = {}\n", efi));
    if let Some(ir) = initrd { content.push_str(&format!("initrd = {}\n", ir)); }
    if let Some(o) = options { content.push_str(&format!("options = {}\n", o)); }
    std::fs::write(&entry_path, content).unwrap_or_else(|e| { eprintln!("error: failed to write {}: {}", entry_path.display(), e); std::process::exit(1); });
    println!("Created entry: {}", entry_path.display());
}

pub fn remove(name: String, boot: Option<String>) {
    let boot = get_boot(boot);
    backup_entries(&boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);
    let found = find_entry_file(dir_path, &name);
    let path = match found { Some(p) => p, None => { eprintln!("error: entry '{}' not found", name); std::process::exit(1); } };
    std::fs::remove_file(&path).unwrap_or_else(|e| { eprintln!("error: failed to remove {}: {}", path.display(), e); std::process::exit(1); });
    println!("Removed entry: {}", path.display());
}

pub fn edit(name: String, boot: Option<String>) {
    let boot = get_boot(boot);
    backup_entries(&boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);
    let found = find_entry_file(dir_path, &name);
    let path = match found { Some(p) => p, None => { eprintln!("error: entry '{}' not found", name); std::process::exit(1); } };

    let editor = std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL")).unwrap_or_else(|_| "nano".to_string());
    let status = Command::new(&editor).arg(&path).status().unwrap_or_else(|e| { eprintln!("error: failed to run editor '{}': {}", editor, e); std::process::exit(1); });
    if !status.success() { eprintln!("warning: editor exited with non-zero status"); }
}

pub fn mark_good(name: String, boot: Option<String>) {
    let boot = get_boot(boot);
    backup_entries(&boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);

    let base_pattern = format!("{}.conf", name);
    let mut found = None;
    if let Ok(read_dir) = std::fs::read_dir(dir_path) {
        for entry in read_dir.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname == base_pattern { println!("Entry '{}' already has no boot counter.", name); return; }
            if fname.starts_with(&format!("{}+", name)) && fname.ends_with(".conf") { found = Some(entry.path()); }
        }
    }
    let path = match found { Some(p) => p, None => { eprintln!("error: entry '{}' not found with boot counter", name); std::process::exit(1); } };
    let new_path = dir_path.join(format!("{}.conf", name));
    std::fs::rename(&path, &new_path).unwrap_or_else(|e| { eprintln!("error: failed to rename: {}", e); std::process::exit(1); });
    println!("Marked '{}' as good.", name);
}

pub fn set_tries(name: String, tries: u32, boot: Option<String>) {
    let boot = get_boot(boot);
    backup_entries(&boot);
    let dir = entries_dir(&boot);
    let dir_path = Path::new(&dir);
    let found = find_entry_file(dir_path, &name);
    let path = match found { Some(p) => p, None => { eprintln!("error: entry '{}' not found", name); std::process::exit(1); } };
    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| { eprintln!("error: failed to read: {}", e); std::process::exit(1); });
    let new_filename = if tries > 0 { format!("{}+{}.conf", name, tries) } else { format!("{}.conf", name) };
    let new_path = dir_path.join(&new_filename);
    if path == new_path { println!("Entry already has {} tries.", tries); return; }
    std::fs::write(&new_path, &content).unwrap_or_else(|e| { eprintln!("error: failed to write: {}", e); std::process::exit(1); });
    if path != new_path { let _ = std::fs::remove_file(&path); }
    println!("Entry '{}' set to {} boot tries.", name, tries);
}

fn find_entry_file(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname == format!("{}.conf", name) || (fname.starts_with(&format!("{}+", name)) && fname.ends_with(".conf")) {
                return Some(entry.path());
            }
        }
    }
    None
}
