#![no_std]
#![no_main]

pub mod vga_buffer;

use core::fmt::Write;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;
use x86_64::instructions::interrupts;

bootloader::entry_point!(kernel_main);

lazy_static! {
    static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe {
            let mut p = SerialPort::new(0x3F8);
            p.init();
            p
        };
        Mutex::new(serial_port)
    };
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial_print_impl(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => {
        $crate::serial_print!("\n");
    };
    ($fmt:expr) => {
        $crate::serial_print!(concat!($fmt, "\n"));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::serial_print!(concat!($fmt, "\n"), $($arg)*);
    };
}

pub fn serial_print_impl(args: core::fmt::Arguments) {
    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).unwrap();
    });
}

fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

fn kernel_main(boot_info: &'static bootloader::BootInfo) -> ! {
    let region_count = boot_info.memory_map.iter().count();
    serial_println!("my_os kernel booted");
    serial_println!("memory map: {} regions", region_count);
    println!("my_os kernel booted");
    println!("memory map: {} regions", region_count);
    serial_println!("kernel ready");
    println!("kernel ready — halting");
    hlt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    println!("KERNEL PANIC: {}", info);
    hlt_loop()
}
