use core::arch::asm;

#[derive(Clone, Copy)]
pub struct PortU8 {
    port: u16,
}

#[derive(Clone, Copy)]
pub struct PortU16 {
    port: u16,
}

impl PortU8 {
    pub const fn new(port: u16) -> Self {
        PortU8 { port }
    }

    pub unsafe fn read(&mut self) -> u8 {
        let value: u8;
        unsafe { asm!("in al, dx", out("al") value, in("dx") self.port, options(nostack, preserves_flags)); }
        value
    }

    pub unsafe fn write(&mut self, value: u8) {
        unsafe { asm!("out dx, al", in("dx") self.port, in("al") value, options(nostack, preserves_flags)); }
    }
}

impl PortU16 {
    pub const fn new(port: u16) -> Self {
        PortU16 { port }
    }

    pub unsafe fn read(&mut self) -> u16 {
        let value: u16;
        unsafe { asm!("in ax, dx", out("ax") value, in("dx") self.port, options(nostack, preserves_flags)); }
        value
    }

    pub unsafe fn write(&mut self, value: u16) {
        unsafe { asm!("out dx, ax", in("dx") self.port, in("ax") value, options(nostack, preserves_flags)); }
    }
}

pub fn hlt() {
    unsafe { asm!("hlt", options(nostack, preserves_flags)); }
}

pub fn spin_loop() {
    unsafe { asm!("pause", options(nostack, preserves_flags)); }
}

pub fn enable_a20() {
    unsafe {
        let mut port = PortU8::new(0x92);
        let val = port.read();
        if val & 0x02 == 0 {
            port.write(val | 0x02);
        }
    }
}

pub fn reset_system() -> ! {
    unsafe {
        let mut status_port = PortU8::new(0x64);
        let mut data_port = PortU8::new(0x60);
        loop {
            let status: u8 = status_port.read();
            if status & 0x02 == 0 {
                data_port.write(0xFE);
                break;
            }
        }
    }
    loop { hlt(); }
}
