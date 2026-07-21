use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::config::{Config, Entry, Keybinds};
use crate::detect;
use crate::vga;
use crate::keyboard;
use crate::keyboard::KeyEvent;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub enum MenuResult {
    Boot(Entry),
    Manual,
    RestoreBackup,
    Shutdown,
}

enum KeyAction {
    Nothing,
    Boot,
    Manual,
    RestoreBackup,
    Shutdown,
}

pub struct Menu {
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub timeout: u64,
    pub keybinds: Keybinds,
}

// ---------------------------------------------------------------------------
// Box-drawing primitives
// ---------------------------------------------------------------------------

const COLS: usize = 80;
const ROWS: usize = 25;

fn box_width(title: &str, lines: &[String]) -> usize {
    let entry_max = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let f1 = "↑↓=boot  m=manual  b=backups";
    let f2 = "f=firmware  r=reboot";
    let max_content = title.chars().count().max(entry_max).max(f1.len()).max(f2.len());
    let desired = max_content + 6;
    (COLS.saturating_sub(2)).min(52.max(desired))
}

fn indent(x: usize) {
    if x > 0 {
        vga::write_str(&" ".repeat(x));
    }
}

fn draw_top(title: &str, w: usize, x: usize) {
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    let dashes = "─".repeat(w.saturating_sub(title.len() + 4));
    indent(x);
    vga::write_str(&format!("┌ {} {}┐\r\n", title, dashes));
}

fn draw_bottom(w: usize, x: usize) {
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    indent(x);
    vga::write_str(&format!("└{}┘\r\n", "─".repeat(w.saturating_sub(2))));
}

fn draw_empty(w: usize, x: usize) {
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    indent(x);
    vga::write_str(&format!("│{}│\r\n", " ".repeat(w.saturating_sub(2))));
}

fn draw_sep(w: usize, x: usize) {
    let inner = w.saturating_sub(4);
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    indent(x);
    vga::write_str("│ ");
    vga::set_color(vga::Color::DarkGray, vga::Color::Black);
    vga::write_str(&"─".repeat(inner));
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str(" │\r\n");
}

fn draw_line(text: &str, w: usize, color: vga::Color, x: usize) {
    let inner = w.saturating_sub(4);
    let tlen = text.chars().count();
    let display: String = text.chars().take(inner).collect();
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    indent(x);
    vga::write_str("│ ");
    vga::set_color(color, vga::Color::Black);
    vga::write_str(&display);
    if tlen < inner {
        vga::write_str(&" ".repeat(inner - tlen));
    }
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str(" │\r\n");
}

fn draw_entry_line(entry: &Entry, selected: bool, w: usize, x: usize) {
    let prefix = if selected { "  > " } else { "    " };
    let counter = entry.boot_counter.map_or(String::new(), |c| {
        if c > 0 { format!(" [{}]", c) } else { String::new() }
    });

    let inner = w.saturating_sub(4);
    let counter_len = counter.chars().count();
    let max_title = inner.saturating_sub(4 + counter_len);
    let title_display: String = entry.title.chars().take(max_title).collect();
    let tlen = title_display.chars().count();
    let pad = max_title.saturating_sub(tlen);
    let left = pad / 2;
    let right = pad - left;

    let bg = if selected { vga::Color::Blue } else { vga::Color::Black };

    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    indent(x);
    vga::write_str("│ ");
    vga::set_color(vga::Color::White, bg);
    vga::write_str(prefix);
    vga::write_str(&" ".repeat(left));
    vga::write_str(&title_display);
    vga::write_str(&" ".repeat(right));

    if !counter.is_empty() {
        vga::set_color(vga::Color::DarkGray, bg);
        vga::write_str(&counter);
    }

    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str(" │\r\n");
}

// ---------------------------------------------------------------------------
// Status dialog
// ---------------------------------------------------------------------------

pub fn show_status(lines: &[(&str, vga::Color)]) {
    let title = format!("nexec-lgc v{}", VERSION);
    let max_line = lines.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0);
    let w = (COLS.saturating_sub(2)).min(46.max(max_line + 8));
    let box_h = lines.len() + 4;
    let start_y = if box_h < ROWS { (ROWS - box_h) / 2 } else { 1 };
    let start_x = (COLS - w) / 2;

    vga::set_cursor(0, start_y);
    draw_top(&title, w, start_x);
    draw_empty(w, start_x);
    for (text, color) in lines {
        draw_line(text, w, *color, start_x);
    }
    draw_empty(w, start_x);
    draw_bottom(w, start_x);
    vga::set_color(vga::Color::Black, vga::Color::Black);
    indent(start_x);
    vga::write_str(&" ".repeat(w));
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str("\r\n");
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw_no_entries(keybinds: &Keybinds) {
    let w = COLS.saturating_sub(2).min(46);
    let box_h = 8;
    let start_y = if box_h < ROWS { (ROWS - box_h) / 2 } else { 1 };
    let start_x = (COLS - w) / 2;
    let title = format!("nexec-lgc v{}", VERSION);

    vga::set_cursor(0, start_y);
    draw_top(&title, w, start_x);
    draw_empty(w, start_x);
    draw_line("No entries detected", w, vga::Color::White, start_x);
    draw_empty(w, start_x);
    draw_line(&format!("{}  manual boot", keybinds.manual), w, vga::Color::DarkGray, start_x);
    draw_line(&format!("{}  firmware setup", keybinds.firmware), w, vga::Color::DarkGray, start_x);
    draw_empty(w, start_x);
    draw_bottom(w, start_x);
    vga::set_color(vga::Color::Black, vga::Color::Black);
    indent(start_x);
    vga::write_str(&" ".repeat(w));
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str("\r\n");
}

fn draw_menu(menu: &Menu, remaining: u64) {
    let title = format!("nexec-lgc v{}", VERSION);
    let kb = &menu.keybinds;
    let foot1 = format!("↑↓=boot  {}=manual  {}=backups", kb.manual, kb.backup);
    let foot2 = format!("{}=firmware  {}=reboot  {}=shutdown", kb.firmware, kb.reboot, kb.shutdown);

    let mut lines: Vec<String> = Vec::new();
    for e in &menu.entries {
        let counter = e.boot_counter.map_or(String::new(), |c| {
            if c > 0 { format!(" [{}]", c) } else { String::new() }
        });
        lines.push(format!("    {}{}", e.title, counter));
    }

    let w = box_width(&title, &lines);
    let inner = w.saturating_sub(4);

    let countdown = if remaining > 0 {
        Some(format!("Auto-boot in {}s", remaining / 10))
    } else {
        None
    };
    let extra = countdown.as_ref().map_or(0, |_| 1);
    let box_h = menu.entries.len() + 8 + extra;
    let start_y = if box_h < ROWS { (ROWS - box_h) / 2 } else { 1 };
    let start_x = (COLS - w) / 2;

    vga::set_cursor(0, start_y);
    draw_top(&title, w, start_x);
    draw_empty(w, start_x);

    for (i, entry) in menu.entries.iter().enumerate() {
        draw_entry_line(entry, i == menu.selected, w, start_x);
    }

    draw_empty(w, start_x);
    draw_sep(w, start_x);

    if let Some(s) = &countdown {
        let pad = inner.saturating_sub(s.chars().count());
        let left = pad / 2;
        let right = pad - left;
        indent(start_x);
        vga::set_color(vga::Color::LightBlue, vga::Color::Black);
        vga::write_str("│ ");
        vga::set_color(vga::Color::White, vga::Color::Black);
        vga::write_str(&" ".repeat(left));
        vga::write_str(s);
        vga::write_str(&" ".repeat(right));
        vga::set_color(vga::Color::LightBlue, vga::Color::Black);
        vga::write_str(" │\r\n");
    }

    indent(start_x);
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str(&format!("│  {}│\r\n", pad_to(&foot1, inner)));

    indent(start_x);
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str(&format!("│  {}│\r\n", pad_to(&foot2, inner)));

    draw_bottom(w, start_x);
    vga::set_color(vga::Color::Black, vga::Color::Black);
    indent(start_x);
    vga::write_str(&" ".repeat(w));
    vga::set_color(vga::Color::LightBlue, vga::Color::Black);
    vga::write_str("\r\n");
}

fn pad_to(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len < width {
        format!("{}{}", s, " ".repeat(width - len))
    } else {
        s.chars().take(width).collect()
    }
}

// ---------------------------------------------------------------------------
// File browser for manual boot
// ---------------------------------------------------------------------------

pub fn browse_files() -> Option<Entry> {
    let entries = detect::scan_all_files();
    if entries.is_empty() {
        show_status(&[
            ("No bootable files found on partition", vga::Color::White),
            ("Press any key to go back...", vga::Color::DarkGray),
        ]);
        keyboard::read_key_blocking();
        return None;
    }

    let mut selected = 0;
    let title = "Manual Boot";

    loop {
        let lines: Vec<String> = entries.iter().map(|e| e.efi_path.clone()).collect();
        let w = box_width(title, &lines);
        let box_h = entries.len() + 6;
        let start_y = if box_h < ROWS { (ROWS - box_h) / 2 } else { 1 };
        let start_x = (COLS - w) / 2;

        vga::set_cursor(0, start_y);
        draw_top(title, w, start_x);
        draw_empty(w, start_x);
        for (i, entry) in entries.iter().enumerate() {
            draw_entry_line(entry, i == selected, w, start_x);
        }
        draw_empty(w, start_x);
        draw_line("Enter=boot  Esc=back", w, vga::Color::DarkGray, start_x);
        draw_bottom(w, start_x);
        vga::set_color(vga::Color::Black, vga::Color::Black);
        indent(start_x);
        vga::write_str(&" ".repeat(w));
        vga::set_color(vga::Color::LightBlue, vga::Color::Black);
        vga::write_str("\r\n");

        match keyboard::read_key_blocking() {
            KeyEvent::Up => { if selected > 0 { selected -= 1; } }
            KeyEvent::Down => { if selected < entries.len().saturating_sub(1) { selected += 1; } }
            KeyEvent::Home => { selected = 0; }
            KeyEvent::End => { selected = entries.len().saturating_sub(1); }
            KeyEvent::Enter => { return Some(entries[selected].clone()); }
            KeyEvent::Escape => { return None; }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Menu implementation
// ---------------------------------------------------------------------------

impl Menu {
    pub fn new(cfg: &Config, detected: Vec<Entry>) -> Self {
        let mut entries: Vec<Entry> = detected;
        for e in &cfg.entries {
            if e.boot_counter == Some(0) {
                continue;
            }
            let is_dup = entries.iter().any(|ee| ee.efi_path == e.efi_path);
            if !is_dup {
                entries.push(e.clone());
            }
        }

        if let Some(order) = &cfg.order {
            let mut ordered: Vec<Entry> = Vec::new();
            let mut remaining: Vec<Entry> = Vec::new();
            for name in order {
                let pos = entries.iter().position(|e| &e.name == name);
                if let Some(i) = pos {
                    ordered.push(entries.remove(i));
                }
            }
            remaining.append(&mut entries);
            ordered.append(&mut remaining);
            entries = ordered;
        }

        let selected = if let Some(ref def) = cfg.default {
            entries.iter().position(|e| Some(&e.name) == Some(def)).unwrap_or(0)
        } else {
            0
        };

        Menu { entries, selected, timeout: cfg.timeout, keybinds: cfg.keybinds.clone() }
    }

    pub fn run(&mut self) -> MenuResult {
        if self.timeout == 0 {
            return self.entries.get(self.selected).cloned()
                .map(MenuResult::Boot)
                .unwrap_or(MenuResult::Manual);
        }

        let mut remaining = self.timeout * 10;
        keyboard::reset();
        vga::clear();

        let mut dirty = true;

        loop {
            if self.entries.is_empty() {
                draw_no_entries(&self.keybinds);
                keyboard::read_key_blocking();
                continue;
            }

            if dirty {
                draw_menu(self, remaining);
                dirty = false;
            }

            if remaining > 0 {
                let event = keyboard::poll_key(100);
                if let Some(ev) = event {
                    remaining = 0;
                    dirty = true;
                    if let Some(action) = self.handle_event(ev) {
                        return action;
                    }
                }
            } else {
                let ev = keyboard::read_key_blocking();
                if let Some(action) = self.handle_event(ev) {
                    return action;
                }
                dirty = true;
            }

            if remaining > 0 {
                let prev = remaining / 10;
                remaining = remaining.saturating_sub(1);
                if remaining / 10 != prev || remaining == 0 {
                    dirty = true;
                }
                if remaining == 0 && !self.entries.is_empty() {
                    return MenuResult::Boot(self.entries[self.selected].clone());
                }
            }
        }
    }

    fn handle_event(&mut self, event: KeyEvent) -> Option<MenuResult> {
        let kb = &self.keybinds;
        match event {
            KeyEvent::Up => {
                if self.selected > 0 { self.selected -= 1; }
            }
            KeyEvent::Down => {
                if self.selected < self.entries.len().saturating_sub(1) { self.selected += 1; }
            }
            KeyEvent::Home => { self.selected = 0; }
            KeyEvent::End => { self.selected = self.entries.len().saturating_sub(1); }
            KeyEvent::Enter => {
                if !self.entries.is_empty() {
                    return Some(MenuResult::Boot(self.entries[self.selected].clone()));
                }
            }
            KeyEvent::Escape => {}
            KeyEvent::Char(c) => {
                let ch = c.to_ascii_lowercase();
                if ch == kb.manual.to_ascii_lowercase() {
                    return Some(MenuResult::Manual);
                }
                if ch == kb.backup.to_ascii_lowercase() {
                    return Some(MenuResult::RestoreBackup);
                }
                if ch == kb.reboot.to_ascii_lowercase() {
                    crate::boot_loader::reset_system();
                }
                if ch == kb.shutdown.to_ascii_lowercase() {
                    return Some(MenuResult::Shutdown);
                }
                if ch == kb.firmware.to_ascii_lowercase() {
                    show_status(&[
                        ("Firmware setup not available in BIOS mode.", vga::Color::Yellow),
                        ("Press any key to continue...", vga::Color::DarkGray),
                    ]);
                    keyboard::read_key_blocking();
                    vga::clear();
                }
                // 1-9 direct entry selection
                if let Some(digit) = c.to_digit(10) {
                    if digit >= 1 && digit <= 9 {
                        let idx = (digit as usize) - 1;
                        if idx < self.entries.len() {
                            self.selected = idx;
                            return Some(MenuResult::Boot(self.entries[self.selected].clone()));
                        }
                    }
                }
            }
        }
        None
    }
}
