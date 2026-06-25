use crate::{
    input, interrupts, print, println, scheduler, scheduler::TaskContext, serial_print,
    serial_println, vga_buffer,
};

pub fn init() {
    serial_println!("[OK] shell ready");
    println!("open_kernel shell ready");
    print_prompt();
}

pub fn run_once() {
    interrupts::poll_keyboard();
    poll_serial_input();

    let line = input::pop_line();

    if let Some(line) = line {
        handle_command(line.trim());
        print_prompt();
    }
}

pub fn task(ctx: &mut TaskContext) {
    run_once();
    ctx.yield_now();
}

fn poll_serial_input() {
    while serial_received() {
        let byte = unsafe { x86_64::instructions::port::Port::<u8>::new(0x3F8).read() };
        if let Some(event) = serial_byte_to_event(byte) {
            handle_input_event(event);
        }
    }
}

fn print_prompt() {
    serial_print!("open_kernel> ");
    print!("open_kernel> ");
}

enum InputEvent {
    Char(char),
    Backspace,
    Enter,
}

fn handle_input_event(event: InputEvent) {
    match event {
        InputEvent::Char(ch) => {
            input::push_char(ch);
            serial_print!("{}", ch);
            print!("{}", ch);
        }
        InputEvent::Backspace => {
            if input::backspace() {
                serial_print!("\x08 \x08");
                print!("\x08");
            }
        }
        InputEvent::Enter => {
            input::commit_line();
            serial_println!();
            println!();
        }
    }
}

fn serial_received() -> bool {
    let status = unsafe { x86_64::instructions::port::Port::<u8>::new(0x3FD).read() };
    status & 1 != 0
}

fn serial_byte_to_event(byte: u8) -> Option<InputEvent> {
    match byte {
        b'\r' | b'\n' => Some(InputEvent::Enter),
        0x08 | 0x7f => Some(InputEvent::Backspace),
        b' '..=b'~' => Some(InputEvent::Char(byte as char)),
        _ => None,
    }
}

fn handle_command(cmd: &str) {
    match cmd {
        "" => {}
        "help" => {
            serial_println!("commands: help, uname, uptime, ps, tasks, clear, reboot");
            println!("commands: help, uname, uptime, ps, tasks, clear, reboot");
        }
        "uname" => {
            serial_println!("open_kernel 0.1.0 x86_64 bare-metal");
            println!("open_kernel 0.1.0 x86_64 bare-metal");
        }
        "uptime" => {
            let ticks = scheduler::uptime_ticks();
            serial_println!("uptime: {} ticks", ticks);
            println!("uptime: {} ticks", ticks);
        }
        "ps" | "tasks" => {
            serial_println!("pid state     runs name");
            println!("pid state     runs name");
            scheduler::snapshot_tasks(|task| {
                serial_println!(
                    "{:>3} {:<8?} {:>4} {}",
                    task.id,
                    task.state,
                    task.runs,
                    task.name
                );
                println!(
                    "{:>3} {:<8?} {:>4} {}",
                    task.id, task.state, task.runs, task.name
                );
            });
        }
        "clear" => {
            vga_buffer::clear_screen();
            serial_print!("\x1b[2J\x1b[H");
        }
        "reboot" => {
            serial_println!("reboot is not wired yet");
            println!("reboot is not wired yet");
        }
        other => {
            serial_println!("unknown command: {}", other);
            println!("unknown command: {}", other);
        }
    }
}
