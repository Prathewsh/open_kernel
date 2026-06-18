#![no_std] // don't link the Rust standard library
#![no_main] // disable the normal entry point chain

use core::panic::PanicInfo;

// Called on panic. A kernel has nowhere to unwind to, so we just loop.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Our entry point. `_start` is the conventional name the linker looks for.
// `extern "C"` uses the C calling convention; no_mangle keeps the name intact.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}
