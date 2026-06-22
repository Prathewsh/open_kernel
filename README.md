# my_os

Minimal bare-metal Rust kernel for `x86_64`.

## What this does

- Builds a `#![no_std]`, `#![no_main]` kernel — no standard library, no OS underneath
- Boots through the `bootloader` crate (multiboot-compatible disk image)
- Outputs to two independent channels simultaneously:
  - **VGA text buffer** (`0xb8000`) — green-on-black text rendered directly to screen memory
  - **UART serial port** (`0x3F8`, COM1) — useful for automated testing and QEMU `-serial` output
- Reads the bootloader-provided **memory map** and reports how many memory regions the firmware discovered
- Disables interrupts around every lock acquisition on both the VGA writer and serial port, preventing deadlocks if an interrupt fires while a lock is held
- Halts the CPU with `hlt` in a tight loop after boot (no busy-waiting, low power)
- On panic: prints the panic location and message to both outputs, then halts cleanly

## Architecture

```
src/
  main.rs         — kernel entry point, serial port init, hlt_loop, panic handler
  vga_buffer.rs   — VGA text driver: Color, ColorCode, ScreenChar, Writer, print!/println! macros
```

### VGA driver ([src/vga_buffer.rs](src/vga_buffer.rs))

Uses `volatile` writes to prevent the compiler from optimising away memory-mapped I/O. Supports scrolling (shifts all rows up by one when the last column is reached or `\n` is written). Non-printable bytes are rendered as `0xfe` (a block character).

### Serial driver ([src/main.rs](src/main.rs))

Wraps `uart_16550::SerialPort` in a `spin::Mutex` via `lazy_static!`. Provides `serial_print!` / `serial_println!` macros that mirror the VGA macros. All writes disable interrupts for the duration of the lock.

### Boot flow

1. `bootloader` sets up the CPU, creates a memory map, and jumps to `kernel_main`
2. `kernel_main` logs the number of detected memory regions and prints a ready message to both outputs
3. CPU enters `hlt_loop` — wakes only on interrupt (none are configured, so it stays halted)

## Dependencies

| Crate | Purpose |
|---|---|
| `bootloader 0.9` | Creates a bootable disk image and passes `BootInfo` to the kernel |
| `uart_16550 0.3` | Safe wrapper around the 16550 UART serial controller |
| `spin 0.9` | `no_std`-compatible spinlock mutex |
| `lazy_static 1.5` | Safe global statics with runtime init in `no_std` |
| `volatile 0.2` | Volatile read/write wrappers for MMIO |
| `x86_64 0.15` | CPU intrinsics: `hlt`, `without_interrupts`, port I/O |

## Setup and running

1. Install Rust nightly:

```bash
rustup toolchain install nightly
```

2. Add the bare-metal target:

```bash
rustup target add x86_64-unknown-none --toolchain nightly
```

3. Install `bootimage`:

```bash
cargo +nightly install bootimage
```

4. Build:

```bash
cargo +nightly build
```

5. Create a bootable image and run in QEMU:

```bash
cargo +nightly bootimage
qemu-system-x86_64 -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-my_os.bin -serial stdio
```

The `-serial stdio` flag pipes COM1 output to your terminal so you can see serial logs alongside the QEMU window.
