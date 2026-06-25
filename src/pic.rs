use pic8259::ChainedPics;
use spin::Mutex;

// Remap IRQs away from the CPU exception range (0–31).
// Master PIC: IRQ0–7  → IDT[32–39]
// Slave PIC:  IRQ8–15 → IDT[40–47]
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,        // IRQ0
    Keyboard = PIC_1_OFFSET + 1, // IRQ1
    Serial1 = PIC_1_OFFSET + 4,  // IRQ4
}

impl InterruptIndex {
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub fn as_usize(self) -> usize {
        usize::from(self as u8)
    }
}

pub fn init() {
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();
        // Unmask IRQ0 timer, IRQ1 keyboard, and IRQ4 COM1 serial.
        pics.write_masks(!(0b0001_0011), u8::MAX);
    }
}
