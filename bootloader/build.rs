use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // Compile the boot assembly stub
    cc::Build::new()
        .file("src/asm/boot.S")
        .compile("boot_asm");

    // Pass linker flags for freestanding binary
    println!("cargo:rustc-link-arg=-nostartfiles");
    println!("cargo:rustc-link-arg=-nostdlib");
    println!("cargo:rustc-link-arg=-no-pie");

    // Pass the linker script
    let linker_script = manifest_dir.parent().unwrap().join("linker.ld");
    println!("cargo:rustc-link-arg=-T{}", linker_script.display());
    println!("cargo:rerun-if-changed=src/asm/boot.S");
    println!("cargo:rerun-if-changed={}", linker_script.display());
}
