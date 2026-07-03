use bootloader_api::{FrameBufferInfo, PixelFormat};
use core::fmt;
use spin::Mutex;

use crate::vga_font::VGA_FONT_8X16;

const GLYPH_WIDTH: usize = 8;
const GLYPH_HEIGHT: usize = 16;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Green,
    LightGreen,
    White,
}

impl Color {
    const fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Black => (0, 0, 0),
            Self::Green => (0, 170, 0),
            Self::LightGreen => (85, 255, 85),
            Self::White => (255, 255, 255),
        }
    }
}

pub struct Writer {
    framebuffer: Option<FrameBufferInfo>,
    column: usize,
    row: usize,
    foreground: Color,
    background: Color,
}

impl Writer {
    const fn new() -> Self {
        Self {
            framebuffer: None,
            column: 0,
            row: 0,
            foreground: Color::White,
            background: Color::Black,
        }
    }

    fn init(&mut self, framebuffer: FrameBufferInfo) {
        assert!(
            framebuffer.address != 0,
            "bootloader provided no framebuffer"
        );
        assert!(
            framebuffer.byte_len >= framebuffer.stride * framebuffer.height * 4,
            "framebuffer is smaller than its mode information"
        );
        self.framebuffer = Some(framebuffer);
        self.clear_screen();
    }

    fn dimensions(&self) -> (usize, usize) {
        let framebuffer = self.framebuffer.expect("framebuffer not initialized");
        (
            framebuffer.width / GLYPH_WIDTH,
            framebuffer.height / GLYPH_HEIGHT,
        )
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x08 => self.backspace(),
            byte => {
                let (columns, rows) = self.dimensions();
                if self.column >= columns {
                    self.new_line();
                }
                if self.row >= rows {
                    self.scroll();
                }

                self.draw_glyph(byte, self.column * GLYPH_WIDTH, self.row * GLYPH_HEIGHT);
                self.column += 1;
            }
        }
    }

    pub fn clear_screen(&mut self) {
        let framebuffer = self.framebuffer.expect("framebuffer not initialized");
        let color = self.background.rgb();
        for y in 0..framebuffer.height {
            for x in 0..framebuffer.width {
                self.write_pixel(x, y, color);
            }
        }
        self.column = 0;
        self.row = 0;
    }

    fn backspace(&mut self) {
        if self.column == 0 {
            return;
        }
        self.column -= 1;
        self.draw_glyph(b' ', self.column * GLYPH_WIDTH, self.row * GLYPH_HEIGHT);
    }

    fn new_line(&mut self) {
        self.column = 0;
        self.row += 1;
        let (_, rows) = self.dimensions();
        if self.row >= rows {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        let framebuffer = self.framebuffer.expect("framebuffer not initialized");
        let row_bytes = framebuffer.stride * 4;
        let scroll_bytes = GLYPH_HEIGHT * row_bytes;
        let retained_bytes = framebuffer.byte_len.saturating_sub(scroll_bytes);
        let base = framebuffer.address as *mut u8;

        unsafe {
            for index in 0..retained_bytes {
                let value = base.add(index + scroll_bytes).read_volatile();
                base.add(index).write_volatile(value);
            }
        }

        let background = self.background.rgb();
        for y in framebuffer.height.saturating_sub(GLYPH_HEIGHT)..framebuffer.height {
            for x in 0..framebuffer.width {
                self.write_pixel(x, y, background);
            }
        }
        self.row = framebuffer.height / GLYPH_HEIGHT - 1;
    }

    fn draw_glyph(&mut self, byte: u8, x: usize, y: usize) {
        let foreground = self.foreground.rgb();
        let background = self.background.rgb();
        let glyph_offset = byte as usize * GLYPH_HEIGHT;

        for glyph_y in 0..GLYPH_HEIGHT {
            let bits = VGA_FONT_8X16[glyph_offset + glyph_y];
            for glyph_x in 0..GLYPH_WIDTH {
                let color = if bits & (0x80 >> glyph_x) != 0 {
                    foreground
                } else {
                    background
                };
                self.write_pixel(x + glyph_x, y + glyph_y, color);
            }
        }
    }

    fn write_pixel(&mut self, x: usize, y: usize, (red, green, blue): (u8, u8, u8)) {
        let framebuffer = self.framebuffer.expect("framebuffer not initialized");
        if x >= framebuffer.width || y >= framebuffer.height {
            return;
        }

        let offset = (y * framebuffer.stride + x) * 4;
        let (first, second, third) = match framebuffer.pixel_format {
            PixelFormat::Rgb => (red, green, blue),
            PixelFormat::Bgr => (blue, green, red),
        };

        unsafe {
            let pixel = (framebuffer.address as *mut u8).add(offset);
            pixel.write_volatile(first);
            pixel.add(1).write_volatile(second);
            pixel.add(2).write_volatile(third);
            pixel.add(3).write_volatile(0);
        }
    }

    pub fn write_string(&mut self, text: &str) {
        for byte in text.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | 0x08 => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }
}

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

pub fn init(framebuffer: FrameBufferInfo) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().init(framebuffer);
    });
}

pub fn clear_screen() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().clear_screen();
    });
}

impl fmt::Write for Writer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write_string(text);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}
