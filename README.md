# my_os

A bare-metal Rust kernel for `x86_64` — no standard library, no operating system underneath. Built from scratch using Rust nightly with a custom target and hand-wired drivers.

---

## Features

- `#![no_std]` / `#![no_main]` — zero runtime, zero libc
- Boots via the `bootloader 0.9` crate (BIOS-compatible disk image)
- Dual output: **VGA text buffer** and **UART serial port** running simultaneously
- Full **GDT + TSS** with a dedicated double-fault interrupt stack
- **IDT** covering CPU exceptions and hardware IRQs
- **8259 PIC** remapped so hardware IRQs land in IDT slots 32–47
- **Virtual memory** — page-table mapper driven by the bootloader memory map
- **Kernel heap** — 256 KiB mapped at `0x4444_4444_0000`, backed by a linked-list allocator; enables `Box`, `Vec`, `String` etc.
- **Cooperative round-robin scheduler** with up to 8 task slots, driven by the PIT timer IRQ
- Keyboard input decoded from PS/2 scancode set 1
- Clean panic handler that prints to both outputs then halts

---

## Source layout

```
src/
  main.rs          kernel entry point · serial macros · panic handler
  vga_buffer.rs    VGA text driver
  gdt.rs           Global Descriptor Table + Task State Segment
  interrupts.rs    IDT · CPU exception handlers · timer/keyboard ISRs
  pic.rs           8259 PIC remapping + EOI helpers
  memory.rs        OffsetPageTable mapper + BootInfoFrameAllocator
  allocator.rs     heap mapping + linked-list global allocator
  scheduler.rs     cooperative round-robin task scheduler
```

---

## Module details

### `vga_buffer.rs`

Writes directly to the VGA text buffer at physical address `0xb8000`. Uses `volatile` crate writes to prevent the compiler from optimising away memory-mapped stores. Supports:

- Full 16-colour foreground/background via `Color` / `ColorCode`
- Automatic line scrolling when the cursor reaches column 80 or a `\n` is written
- Non-printable bytes rendered as `0xfe` (a filled block)
- `print!` / `println!` macros that mirror the standard library interface
- A `spin::Mutex`-guarded global `WRITER` with interrupts disabled during every lock, preventing deadlocks if a print macro is called from an interrupt handler

### `gdt.rs`

Defines a minimal Global Descriptor Table with kernel code and data segments plus a Task State Segment. The TSS provides a separate interrupt stack for the double-fault handler (`IST[0]`), so a double fault triggered by a corrupt or overflowed stack does not immediately triple-fault. The GDT and TSS are stored in `lazy_static!` statics and loaded with `lgdt` / `ltr` during `gdt::init()`.

### `interrupts.rs`

Builds and loads the IDT. Handlers:

| Vector | Source | Action |
|--------|--------|--------|
| `#BP` (3) | Breakpoint | Logs stack frame to serial; used as boot self-test |
| `#DF` (8) | Double fault | Panics with stack frame (runs on dedicated IST stack) |
| `#PF` (14) | Page fault | Prints faulting address + error code from CR2, then panics |
| `#GP` (13) | General protection | Panics with error code |
| IRQ0 (32) | PIT timer | Calls `scheduler::on_timer_tick()`, sends EOI |
| IRQ1 (33) | PS/2 keyboard | Reads scancode from port `0x60`, translates to ASCII, prints; sends EOI |

The keyboard handler translates scancode set 1: letters a–z, digits 0–9, space, and Enter.

### `pic.rs`

Reinitialises the two chained 8259 PICs, remapping IRQ0–7 to IDT[32–39] and IRQ8–15 to IDT[40–47] so hardware interrupts don't collide with CPU exception vectors. Exposes `InterruptIndex::Timer` and `InterruptIndex::Keyboard` as typed `u8` constants for use in the IDT and EOI calls.

### `memory.rs`

On boot the `bootloader` crate identity-maps all physical memory at a fixed virtual offset and passes that offset in `BootInfo`. `memory::init` uses this to build an `OffsetPageTable` — a page-table walker that translates virtual → physical addresses by adding the offset. `BootInfoFrameAllocator` walks the bootloader memory map and hands out `Usable` 4 KiB physical frames one at a time.

### `allocator.rs`

Maps 64 contiguous 4 KiB pages (256 KiB total) into the virtual range starting at `0x4444_4444_0000`, then initialises a `linked_list_allocator::LockedHeap` over that range. Registered as `#[global_allocator]`, this makes `alloc` types available anywhere in the kernel.

Heap constants:

| Name | Value |
|------|-------|
| `HEAP_START` | `0x4444_4444_0000` |
| `HEAP_SIZE` | `262144` (256 KiB) |

### `scheduler.rs`

A cooperative round-robin scheduler with a fixed-size task table (`MAX_TASKS = 8`).

**Task model**

Each task is a plain function `fn(&mut TaskContext)`. The `TaskContext` passed in gives the task its ID and the current scheduler tick. Tasks signal intent by calling:

- `ctx.yield_now()` — give up the CPU; stay Ready for the next round
- `ctx.finish()` — mark the task as Finished; it will never run again

Tasks that return without calling either method are also rescheduled (treated as an implicit yield with no rotation).

**Scheduler loop (`run()`)**

```
loop {
    Phase 1  acquire lock → pick next Ready task → release lock
    Phase 2  call task function  (lock NOT held)
    Phase 3  acquire lock → commit task state → release lock
}
```

Releasing the lock before calling the task function is critical: the PIT timer ISR (`on_timer_tick`) also acquires the scheduler mutex to advance the tick counter. If the lock were held across the task call, any timer interrupt during task execution would spin forever — a deadlock. By splitting into three phases, the ISR can always make progress.

When no task is runnable the scheduler uses `enable_and_hlt()`, which enables interrupts and halts the CPU in one atomic instruction sequence. This avoids the race where a wakeup interrupt arrives between a separate `sti` and `hlt`.

**Built-in tasks**

| Name | Behaviour |
|------|-----------|
| `idle` | Logs to serial every 32 ticks; always yields |
| `worker-a` | Logs to VGA + serial every 16 ticks; always yields |
| `worker-b` (logger) | Logs timer-tick events from the ISR flag; finishes after tick 128 |

**Timer integration**

The PIT fires IRQ0 at roughly 18 Hz. Each interrupt:
1. Increments `BOOT_TICK_COUNT` (atomic)
2. Sets `SCHEDULER_TICKED` flag (atomic, `Release` ordering)
3. Locks the scheduler and increments its internal tick counter

The logger task reads `SCHEDULER_TICKED` with `Acquire` ordering so it observes the updated count.

---

## Boot sequence

```
kernel_main()
  │
  ├─ gdt::init()           load GDT + TSS
  ├─ interrupts::init()    load IDT
  ├─ pic::init()           remap and initialise 8259 PIC
  ├─ int3                  breakpoint self-test (verifies IDT works)
  │
  ├─ memory::init()        build OffsetPageTable from bootloader physical-memory offset
  ├─ allocator::init_heap  map 256 KiB heap, install global allocator
  │
  ├─ scheduler::init()     register idle / worker-a / worker-b tasks
  ├─ sti                   enable hardware interrupts
  │
  └─ scheduler::run()      cooperative loop forever
```

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `bootloader` | 0.9 | BIOS-compatible bootloader; passes `BootInfo` (memory map + physical offset) |
| `uart_16550` | 0.3 | Safe 16550 UART driver; COM1 serial output |
| `spin` | 0.9 | `no_std` spinlock mutex |
| `lazy_static` | 1.5 | Runtime-initialised `no_std` globals |
| `volatile` | 0.2 | Volatile MMIO read/write wrappers |
| `x86_64` | 0.15 | CPU intrinsics, page tables, IDT/GDT structures |
| `pic8259` | 0.10 | Chained 8259 PIC driver |
| `linked_list_allocator` | 0.10 | `no_std` heap allocator |

---

## Building and running

### Prerequisites

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
cargo +nightly install bootimage
```

QEMU must be installed and `qemu-system-x86_64` on your `PATH`.

### Build

```bash
cargo build
```

The `.cargo/config.toml` sets the default target to `x86_64-unknown-none` and uses `-Z build-std` to compile `core`, `alloc`, and `compiler_builtins` from source, so no extra `--target` flag is needed.

### Run in QEMU

```bash
cargo bootimage
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-my_os.bin \
  -serial stdio
```

`-serial stdio` pipes COM1 to your terminal. You will see structured `[OK]` boot logs in the terminal and the VGA output in the QEMU window.

### Keyboard input

With QEMU focused, type any alphanumeric key — the scancode is translated and echoed to both outputs. Unsupported scancodes are silently ignored.
