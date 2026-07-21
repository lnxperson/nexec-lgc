use core::fmt;

const VGA_BUFFER: *mut u16 = 0xB8000 as *mut u16;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightMagenta = 13,
    Yellow = 14,
    White = 15,
}

fn color_code(fg: Color, bg: Color) -> u8 {
    (bg as u8) << 4 | (fg as u8)
}

pub struct VgaWriter {
    col: usize,
    row: usize,
    fg: Color,
    bg: Color,
}

static mut WRITER: VgaWriter = VgaWriter {
    col: 0,
    row: 0,
    fg: Color::LightBlue,
    bg: Color::Black,
};

pub fn init() {
    unsafe {
        WRITER = VgaWriter {
            col: 0,
            row: 0,
            fg: Color::LightBlue,
            bg: Color::Black,
        };
    }
    clear();
}

pub fn set_color(fg: Color, bg: Color) {
    unsafe { WRITER.fg = fg; WRITER.bg = bg; }
}

pub fn set_cursor(col: usize, row: usize) {
    unsafe { WRITER.col = col; WRITER.row = row; }
}

fn vga_entry(c: u8, color: u8) -> u16 {
    (color as u16) << 8 | c as u16
}

fn write_char(col: usize, row: usize, c: u8, fg: Color, bg: Color) {
    let idx = row * VGA_WIDTH + col;
    let color = color_code(fg, bg);
    unsafe {
        VGA_BUFFER.wrapping_add(idx).write_volatile(vga_entry(c, color));
    }
}

pub fn clear() {
    for row in 0..VGA_HEIGHT {
        for col in 0..VGA_WIDTH {
            write_char(col, row, b' ', Color::LightBlue, Color::Black);
        }
    }
    unsafe { WRITER.col = 0; WRITER.row = 0; }
}

pub fn write_str(s: &str) {
    let fg = unsafe { WRITER.fg };
    let bg = unsafe { WRITER.bg };
    let mut col = unsafe { WRITER.col };
    let mut row = unsafe { WRITER.row };

    for byte in s.bytes() {
        match byte {
            b'\r' => { col = 0; }
            b'\n' => {
                col = 0;
                row += 1;
                if row >= VGA_HEIGHT {
                    scroll();
                    row = VGA_HEIGHT - 1;
                }
            }
            c => {
                write_char(col, row, c, fg, bg);
                col += 1;
                if col >= VGA_WIDTH {
                    col = 0;
                    row += 1;
                    if row >= VGA_HEIGHT {
                        scroll();
                        row = VGA_HEIGHT - 1;
                    }
                }
            }
        }
    }

    unsafe { WRITER.col = col; WRITER.row = row; }
}

fn scroll() {
    for row in 1..VGA_HEIGHT {
        for col in 0..VGA_WIDTH {
            let from = row * VGA_WIDTH + col;
            let to = (row - 1) * VGA_WIDTH + col;
            unsafe {
                let val = VGA_BUFFER.wrapping_add(from).read_volatile();
                VGA_BUFFER.wrapping_add(to).write_volatile(val);
            }
        }
    }
    let last_row = VGA_HEIGHT - 1;
    for col in 0..VGA_WIDTH {
        write_char(col, last_row, b' ', Color::LightBlue, Color::Black);
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                b'\r' => self.col = 0,
                b'\n' => {
                    self.col = 0;
                    self.row += 1;
                    if self.row >= VGA_HEIGHT {
                        scroll_impl(self.fg, self.bg);
                        self.row = VGA_HEIGHT - 1;
                    }
                }
                c => {
                    write_char(self.col, self.row, c, self.fg, self.bg);
                    self.col += 1;
                    if self.col >= VGA_WIDTH {
                        self.col = 0;
                        self.row += 1;
                        if self.row >= VGA_HEIGHT {
                            scroll_impl(self.fg, self.bg);
                            self.row = VGA_HEIGHT - 1;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn scroll_impl(fg: Color, bg: Color) {
    for row in 1..VGA_HEIGHT {
        for col in 0..VGA_WIDTH {
            let from = row * VGA_WIDTH + col;
            let to = (row - 1) * VGA_WIDTH + col;
            unsafe {
                let val = VGA_BUFFER.wrapping_add(from).read_volatile();
                VGA_BUFFER.wrapping_add(to).write_volatile(val);
            }
        }
    }
    for col in 0..VGA_WIDTH {
        write_char(col, VGA_HEIGHT - 1, b' ', fg, bg);
    }
}
