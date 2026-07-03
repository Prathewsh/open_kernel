#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod allocator;
pub mod gdt;
pub mod input;
pub mod interrupts;
pub mod memory;
pub mod pic;
pub mod scheduler;
pub mod shell;
pub mod vga_buffer;
pub mod vga_font;
pub mod rtc;
pub mod vfs;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt::Write;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;
use x86_64::{instructions::interrupts as cpu_irq, VirtAddr};

#[no_mangle]
pub extern "sysv64" fn _start(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    kernel_main(boot_info)
}

lazy_static! {
    static ref SERIAL1: Mutex<SerialPort> = {
        let mut p = unsafe { SerialPort::new(0x3F8) };
        p.init();
        unsafe {
            x86_64::instructions::port::Port::<u8>::new(0x3F9).write(0x01);
            x86_64::instructions::port::Port::<u8>::new(0x3FC).write(0x0B);
        }
        Mutex::new(p)
    };
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => { $crate::serial_print_impl(format_args!($($arg)*)); };
}

#[macro_export]
macro_rules! serial_println {
    ()                       => { $crate::serial_print!("\n"); };
    ($fmt:expr)              => { $crate::serial_print!(concat!($fmt, "\n")); };
    ($fmt:expr, $($arg:tt)*) => { $crate::serial_print!(concat!($fmt, "\n"), $($arg)*); };
}

pub fn serial_print_impl(args: core::fmt::Arguments) {
    cpu_irq::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).unwrap();
    });
}

pub(crate) fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // UEFI page tables mark various pages read-only. Clear CR0.WP early so
    // we can write to the GOP framebuffer and page table entries.
    unsafe {
        let cr0 = x86_64::registers::control::Cr0::read_raw();
        x86_64::registers::control::Cr0::write_raw(
            cr0 & !x86_64::registers::control::Cr0Flags::WRITE_PROTECT.bits(),
        );
    }

    // Continue using the GOP framebuffer that OVMF configured and displayed.
    vga_buffer::init(boot_info.framebuffer);

    serial_println!("open_kernel booting...");

    // ── CPU / interrupt setup ─────────────────────────────────────────────
    gdt::init();
    serial_println!("[OK] GDT + TSS");

    interrupts::init();
    serial_println!("[OK] IDT");

    pic::init();
    serial_println!("[OK] PIC  (IRQs → IDT[32..47])");

    cpu_irq::int3(); // self-test
    serial_println!("[OK] exception handling");

    // ── Memory setup ──────────────────────────────────────────────────────

    let phys_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // Re-enable write protection now that page tables are set up
    unsafe {
        let cr0 = x86_64::registers::control::Cr0::read_raw();
        x86_64::registers::control::Cr0::write_raw(
            cr0 | x86_64::registers::control::Cr0Flags::WRITE_PROTECT.bits(),
        );
    }
    serial_println!(
        "[OK] heap   ({} KiB at {:#x})",
        allocator::HEAP_SIZE / 1024,
        allocator::HEAP_START
    );

    // ── Heap smoke test ───────────────────────────────────────────────────
    let boxed: Box<u64> = Box::new(0xdeadbeef);
    serial_println!("[OK] Box<u64>  = {:#x}  (@ {:p})", *boxed, boxed);

    let mut v: Vec<u32> = Vec::new();
    for i in 0..8 {
        v.push(i * i);
    }
    serial_println!("[OK] Vec<u32>  = {:?}", v);

    let s = String::from("hello from the kernel heap");
    serial_println!("[OK] String    = \"{}\"", s);

    // ── Ready ─────────────────────────────────────────────────────────────
    let region_count = boot_info.memory_map.iter().count();
    serial_println!("[  ] memory map: {} regions", region_count);

    scheduler::init();
    serial_println!("[OK] scheduler initialized");

    shell::init();

    cpu_irq::enable();
    serial_println!("[OK] interrupts enabled — scheduler running");

    println!("focus this window and type: help");

    scheduler::run()
}

#[alloc_error_handler]
fn alloc_error(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation failed: {:?}", layout);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    println!("KERNEL PANIC: {}", info);
    hlt_loop()
}
