use core::cell::UnsafeCell;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::ports::PortU8;

struct KbdInner {
    inner: UnsafeCell<Option<Keyboard<layouts::Us104Key, ScancodeSet1>>>,
}

unsafe impl Sync for KbdInner {}

static KEYBOARD: KbdInner = KbdInner { inner: UnsafeCell::new(None) };

pub enum KeyEvent {
    Char(char),
    Up,
    Down,
    Home,
    End,
    Enter,
    Escape,
}

pub fn init() {
    unsafe {
        *KEYBOARD.inner.get() = Some(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));
    }
}

pub fn read_key_blocking() -> KeyEvent {
    let mut data_port = PortU8::new(0x60);
    let mut status_port = PortU8::new(0x64);

    loop {
        let status: u8 = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            let scancode: u8 = unsafe { data_port.read() };
            if scancode & 0x80 != 0 { continue; }
            if let Some(ev) = scancode_to_event(scancode) { return ev; }
            let keyboard = unsafe { (*KEYBOARD.inner.get()).as_mut().unwrap() };
            if let Ok(Some(kbd_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(kbd_event) {
                    match key {
                        DecodedKey::Unicode(c) => return KeyEvent::Char(c),
                        DecodedKey::RawKey(_) => continue,
                    }
                }
            }
        }
    }
}

pub fn poll_key(delay_ms: u64) -> Option<KeyEvent> {
    let mut data_port = PortU8::new(0x60);
    let mut status_port = PortU8::new(0x64);

    for _ in 0..delay_ms {
        let status: u8 = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            let scancode: u8 = unsafe { data_port.read() };
            if scancode & 0x80 != 0 { continue; }
            if let Some(ev) = scancode_to_event(scancode) { return Some(ev); }
            let keyboard = unsafe { (*KEYBOARD.inner.get()).as_mut().unwrap() };
            if let Ok(Some(kbd_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(kbd_event) {
                    match key {
                        DecodedKey::Unicode(c) => return Some(KeyEvent::Char(c)),
                        DecodedKey::RawKey(_) => continue,
                    }
                }
            }
        }
        for _ in 0..500000 { crate::ports::spin_loop(); }
    }
    None
}

fn scancode_to_event(scancode: u8) -> Option<KeyEvent> {
    match scancode {
        0x48 => Some(KeyEvent::Up),
        0x50 => Some(KeyEvent::Down),
        0x47 => Some(KeyEvent::Home),
        0x4F => Some(KeyEvent::End),
        0x1C => Some(KeyEvent::Enter),
        0x01 => Some(KeyEvent::Escape),
        _ => None,
    }
}

pub fn reset() {
    unsafe {
        if let Some(ref mut kb) = *KEYBOARD.inner.get() { kb.clear(); }
    }
}
