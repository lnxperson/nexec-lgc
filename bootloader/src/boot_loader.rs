use alloc::string::String;
use alloc::format;

use crate::config::Entry;
use crate::disk::{Disk, Fat32};
use crate::ports;
use crate::util;

pub fn boot_entry(entry: &Entry) -> bool {
    let mut disk = match Disk::open() {
        Some(d) => d,
        None => return false,
    };
    let partition = match disk.find_boot_partition() {
        Some(p) => p,
        None => return false,
    };
    let mut fs = match Fat32::new(&mut disk, partition) {
        Some(f) => f,
        None => return false,
    };

    let normalized = util::normalize_path(&entry.efi_path);
    let kernel_data = match fs.read_file(&normalized) {
        Some(d) => d,
        None => return false,
    };

    if kernel_data.len() < 4096 {
        return false;
    }

    let setup_sects = if kernel_data[0x1F1] == 0 { 4u16 } else { kernel_data[0x1F1] as u16 };
    let setup_size = (setup_sects as usize + 1) * 512;

    const KERNEL_LOAD_ADDR: *mut u8 = 0x100000 as *mut u8;
    const SETUP_LOAD_ADDR: *mut u8 = 0x90000 as *mut u8;

    unsafe {
        core::ptr::copy_nonoverlapping(kernel_data.as_ptr(), SETUP_LOAD_ADDR, setup_size.min(kernel_data.len()));

        let payload = &kernel_data[setup_size..];
        core::ptr::copy_nonoverlapping(payload.as_ptr(), KERNEL_LOAD_ADDR, payload.len());

        // Set up kernel command line
        let cmdline = build_cmdline(entry);
        let cmdline_addr = 0x10000 as *mut u8;
        let cmdline_bytes = cmdline.as_bytes();
        let cmdline_len = cmdline_bytes.len().min(2047);
        core::ptr::copy_nonoverlapping(cmdline_bytes.as_ptr(), cmdline_addr, cmdline_len);
        core::ptr::write(cmdline_addr.add(cmdline_len), 0);

        // Set setup header fields
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x1FA) as *mut u16, 0xFFFFu16);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x211) as *mut u8, 0x01u8);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x1F4) as *mut u16, 0x8000u16);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x214) as *mut u32, 0x100000u32);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x224) as *mut u16, 0xE000u16);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x228) as *mut u32, 0x10000u32);
        core::ptr::write_unaligned(SETUP_LOAD_ADDR.add(0x23C) as *mut u32, cmdline_len as u32);
    }

    ports::enable_a20();

    // Jump to the setup code (at offset 0x200 from 0x90000)
    let setup_entry: extern "C" fn() = unsafe { core::mem::transmute(0x90200 as *const ()) };
    setup_entry();
    false
}

fn build_cmdline(entry: &Entry) -> String {
    let mut cmdline = entry.options.clone().unwrap_or_default();
    for initrd_path in &entry.initrd {
        if !cmdline.is_empty() { cmdline.push(' '); }
        cmdline.push_str(&format!("initrd={}", initrd_path));
    }
    cmdline
}

pub fn wait_for_key() {
    crate::keyboard::read_key_blocking();
}

pub fn reset_system() -> ! {
    ports::reset_system()
}
