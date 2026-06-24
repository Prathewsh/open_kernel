# open_kernel

A bare-metal Rust kernel for `x86_64` - no standard library, no operating system underneath. Built from scratch using Rust nightly with a custom target, hand-wired drivers, and a fully custom UEFI bootloader.

This project is currently a kernel, not a full desktop/server operating system. It boots, initializes core hardware and memory services, and runs a small cooperative task scheduler inside the kernel itself.

## What it does today

- Boots into a `#![no_std]`, `#![no_main]` kernel via a custom UEFI bootloader.
- Initializes core x86_64 architecture features: GDT, TSS, IDT, and PIC.
- Sets up virtual memory paging and a 256 KiB kernel heap.
- Uses a cooperative round-robin scheduler for kernel tasks.
- Handles hardware interrupts for timer and keyboard (PS/2 scancode set 1).
- Provides a tiny interactive shell with basic commands (`help`, `uname`, `uptime`, `ps`, etc.).
- Outputs to both a VGA text buffer and a UART serial port simultaneously.

## Building and running

### Prerequisites

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup target add x86_64-unknown-uefi
```

You also need `mtools` and QEMU (with OVMF firmware) installed:
- **macOS:** `brew install mtools qemu`
- **Linux:** `sudo apt install mtools qemu-system-x86 ovmf`

### Build and Run

The simplest way to build the kernel, bootloader, and run the OS in QEMU is to use the provided `run.sh` script:

```bash
./run.sh
```

This script will automatically:
- Build the kernel and custom UEFI bootloader
- Create a FAT32 disk image (`build/uefi_disk.img`)
- Copy the compiled files into the disk image
- Start the OS in QEMU with COM1 logs written to `serial.log`

## Wishlist

Roadmap of things that would turn this kernel into a more complete operating system:

- Preemptive multitasking and real context switching with per-task CPU state.
- Separate stacks for each task.
- User mode support and privilege switching.
- System calls so user programs can request kernel services.
- Process creation, termination, and waiting.
- Per-process virtual memory spaces and memory isolation.
- ELF loading for executable binaries.
- A file system layer.
- Better keyboard input handling, including modifiers and special keys.
- Basic driver model for storage and graphics.
- Network stack support.
- Multicore bring-up and scheduler scaling.
