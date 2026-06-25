use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::gdt;
use crate::input;
use crate::pic::{InterruptIndex, PICS};
use crate::scheduler;
use crate::{print, println, serial_print, serial_println};

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
        idt[InterruptIndex::Serial1.as_u8()].set_handler_fn(serial_handler);

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
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8())
    }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // Read the scancode — this also clears the keyboard controller's buffer.
    let scancode: u8 = unsafe { Port::new(0x60).read() };

    // Translate scancode set 1 into shell input events.
    if let Some(event) = scancode_to_event(scancode) {
        handle_key_event(event);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8())
    }
}

extern "x86-interrupt" fn serial_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    while serial_received() {
        let byte: u8 = unsafe { Port::new(0x3F8).read() };
        if let Some(event) = serial_byte_to_event(byte) {
            handle_key_event(event);
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Serial1.as_u8())
    }
}

enum KeyEvent {
    Char(char),
    Backspace,
    Enter,
}

fn handle_key_event(event: KeyEvent) {
    match event {
        KeyEvent::Char(ch) => {
            input::push_char(ch);
            serial_print!("{}", ch);
            print!("{}", ch);
        }
        KeyEvent::Backspace => {
            if input::backspace() {
                serial_print!("\x08 \x08");
                print!("\x08");
            }
        }
        KeyEvent::Enter => {
            input::commit_line();
            serial_println!();
            println!();
        }
    }
}

pub fn poll_keyboard() {
    while keyboard_data_ready() {
        let scancode = unsafe { x86_64::instructions::port::Port::<u8>::new(0x60).read() };
        if let Some(event) = scancode_to_event(scancode) {
            handle_key_event(event);
        }
    }
}

fn keyboard_data_ready() -> bool {
    let status = unsafe { x86_64::instructions::port::Port::<u8>::new(0x64).read() };
    status & 1 != 0
}

fn serial_received() -> bool {
    use x86_64::instructions::port::Port;

    let status: u8 = unsafe { Port::new(0x3FD).read() };
    status & 1 != 0
}

fn serial_byte_to_event(byte: u8) -> Option<KeyEvent> {
    match byte {
        b'\r' | b'\n' => Some(KeyEvent::Enter),
        0x08 | 0x7f => Some(KeyEvent::Backspace),
        b' '..=b'~' => Some(KeyEvent::Char(byte as char)),
        _ => None,
    }
}

/// Minimal scancode-set-1 → input events for keys a–z, 0–9, space, Enter, and Backspace.
fn scancode_to_event(scancode: u8) -> Option<KeyEvent> {
    // High bit set = key release; ignore it.
    if scancode & 0x80 != 0 {
        return None;
    }
    let event = match scancode {
        0x02..=0x0A => KeyEvent::Char((b'1' + (scancode - 0x02)) as char), // 1–9
        0x0B => KeyEvent::Char('0'),
        0x10 => KeyEvent::Char('q'),
        0x11 => KeyEvent::Char('w'),
        0x12 => KeyEvent::Char('e'),
        0x13 => KeyEvent::Char('r'),
        0x14 => KeyEvent::Char('t'),
        0x15 => KeyEvent::Char('y'),
        0x16 => KeyEvent::Char('u'),
        0x17 => KeyEvent::Char('i'),
        0x18 => KeyEvent::Char('o'),
        0x19 => KeyEvent::Char('p'),
        0x1E => KeyEvent::Char('a'),
        0x1F => KeyEvent::Char('s'),
        0x20 => KeyEvent::Char('d'),
        0x21 => KeyEvent::Char('f'),
        0x22 => KeyEvent::Char('g'),
        0x23 => KeyEvent::Char('h'),
        0x24 => KeyEvent::Char('j'),
        0x25 => KeyEvent::Char('k'),
        0x26 => KeyEvent::Char('l'),
        0x2C => KeyEvent::Char('z'),
        0x2D => KeyEvent::Char('x'),
        0x2E => KeyEvent::Char('c'),
        0x2F => KeyEvent::Char('v'),
        0x30 => KeyEvent::Char('b'),
        0x31 => KeyEvent::Char('n'),
        0x32 => KeyEvent::Char('m'),
        0x39 => KeyEvent::Char(' '),
        0x1C => KeyEvent::Enter,
        0x0E => KeyEvent::Backspace,
        _ => return None,
    };
    Some(event)
}
