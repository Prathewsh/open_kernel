use crate::{
    input::INPUT, interrupts, print, println, scheduler, scheduler::TaskContext, serial_print,
    serial_println, vga_buffer,
};

pub fn init() {
    serial_println!("[OK] shell ready");
    println!("my_os shell ready");
    print_prompt();
}

pub fn run_once() {
    interrupts::poll_keyboard();
    poll_serial_input();

    let line = {
        let mut input = INPUT.lock();
        input.pop_line()
    };

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
    print!("myos> ");
}

enum InputEvent {
    Char(char),
    Backspace,
    Enter,
}

fn handle_input_event(event: InputEvent) {
    match event {
        InputEvent::Char(ch) => {
            INPUT.lock().push_char(ch);
            serial_print!("{}", ch);
            print!("{}", ch);
        }
        InputEvent::Backspace => {
            if INPUT.lock().backspace() {
                serial_print!("\x08 \x08");
                print!("\x08");
            }
        }
        InputEvent::Enter => {
            INPUT.lock().commit_line();
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
            println!("commands: help, uname, uptime, ps, tasks, clear, reboot");
        }
        "uname" => {
            println!("my_os 0.1.0 x86_64 bare-metal");
        }
        "uptime" => {
            println!("uptime: {} ticks", scheduler::uptime_ticks());
        }
        "ps" | "tasks" => {
            println!("pid state     runs name");
            scheduler::snapshot_tasks(|task| {
                println!("{:>3} {:<8?} {:>4} {}", task.id, task.state, task.runs, task.name);
            });
        }
        "clear" => {
            vga_buffer::clear_screen();
        }
        "reboot" => {
            println!("reboot is not wired yet");
        }
        other => {
            println!("unknown command: {}", other);
        }
    }
}
