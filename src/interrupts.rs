use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::gdt;
use crate::pic::{InterruptIndex, PICS};
use crate::scheduler;
use crate::{println, serial_println};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // CPU exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // Hardware interrupts
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}

// ── CPU exception handlers ────────────────────────────────────────────────────

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
    println!("EXCEPTION: BREAKPOINT");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    serial_println!("EXCEPTION: PAGE FAULT");
    serial_println!("  Accessed address : {:?}", Cr2::read());
    serial_println!("  Error code       : {:?}", error_code);
    serial_println!("{:#?}", stack_frame);
    panic!("unhandled page fault");
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "EXCEPTION: GENERAL PROTECTION FAULT (error={:#x})\n{:#?}",
        error_code, stack_frame
    );
}

// ── Hardware interrupt handlers ───────────────────────────────────────────────

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    // Every tick: acknowledge the interrupt so the PIC sends the next one.
    scheduler::on_timer_tick();
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()) }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // Read the scancode — this also clears the keyboard controller's buffer.
    let scancode: u8 = unsafe { Port::new(0x60).read() };

    // Translate scancode set 1 to a printable character (bare minimum subset).
    if let Some(ch) = scancode_to_char(scancode) {
        serial_println!("KEY: {}", ch);
        println!("KEY: {}", ch);
    }

    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()) }
}

/// Minimal scancode-set-1 → ASCII for keys a–z, 0–9, space, and Enter.
fn scancode_to_char(scancode: u8) -> Option<char> {
    // High bit set = key release; ignore it.
    if scancode & 0x80 != 0 {
        return None;
    }
    let ch = match scancode {
        0x02..=0x0A => (b'1' + (scancode - 0x02)) as char, // 1–9
        0x0B         => '0',
        0x10         => 'q', 0x11 => 'w', 0x12 => 'e', 0x13 => 'r',
        0x14         => 't', 0x15 => 'y', 0x16 => 'u', 0x17 => 'i',
        0x18         => 'o', 0x19 => 'p',
        0x1E         => 'a', 0x1F => 's', 0x20 => 'd', 0x21 => 'f',
        0x22         => 'g', 0x23 => 'h', 0x24 => 'j', 0x25 => 'k',
        0x26         => 'l',
        0x2C         => 'z', 0x2D => 'x', 0x2E => 'c', 0x2F => 'v',
        0x30         => 'b', 0x31 => 'n', 0x32 => 'm',
        0x39         => ' ',
        0x1C         => '\n',
        _            => return None,
    };
    Some(ch)
}
