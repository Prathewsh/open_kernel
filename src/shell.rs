use alloc::{format, vec::Vec};
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

use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    static ref PROMPT: Mutex<alloc::string::String> = Mutex::new(alloc::string::String::from("root"));
}

fn print_prompt() {
    let p = PROMPT.lock();
    let pwd = crate::vfs::get_current_directory();
    serial_print!("{}:{}> ", *p, pwd);
    print!("{}:{}> ", *p, pwd);
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
    let (cmd_name, args) = match cmd.find(' ') {
        Some(idx) => (&cmd[..idx], cmd[idx + 1..].trim()),
        None => (cmd, ""),
    };

    match cmd_name {
        "" => {}
        "help" => {
            let help_text = "uname       : prints system information
sysinfo     : shows detailed system info
meminfo     : displays heap memory stats
cpuid       : displays CPU hardware info
vmap        : displays per-process virtual memory spaces & page tables
drivers     : lists storage & graphics drivers in driver model
ifconfig    : displays network stack interface configuration
ping <ip>   : sends ICMP echo requests over network stack
netstat     : displays active network sockets
modifiers   : displays keyboard modifier states (Shift/Ctrl/Alt)
uptime      : shows system uptime (HH:MM:SS)
time        : shows current time
date        : shows current date
cd <dir>    : changes current working directory
pwd         : prints current working directory
ls [path]   : lists files and directories
tree        : displays directory tree
cat <file>  : displays content of a file
touch <file>: creates empty file
write f txt : writes text content to file
mkdir <dir> : creates directory
rm <path>   : removes file or directory
echo <msg>  : prints argument text
color <thm> : sets VGA color theme (default|matrix|amber|cyberpunk)
prompt <str>: changes shell prompt string
matrix      : displays green ASCII matrix digital rain
guess <num> : number guessing game (1-10)
calc <expr> : evaluates basic arithmetic (e.g. calc 12 + 34)
ps/tasks    : list running tasks
spawn [name]: creates new background task
kill <pid>  : terminates running task by PID
clear       : clears the screen
panic       : triggers kernel panic test
reboot      : restarts the system
shutdown    : powers off the system";
            serial_println!("{}", help_text);
            println!("{}", help_text);
        }
        "drivers" => {
            crate::driver::print_drivers();
        }
        "ifconfig" => {
            crate::net::print_ifconfig();
        }
        "ping" => {
            let target = if args.is_empty() { "127.0.0.1" } else { args };
            crate::net::ping(target);
        }
        "netstat" => {
            crate::net::print_netstat();
        }
        "vmap" => {
            crate::process_memory::print_vmap();
        }
        "modifiers" => {
            let mods = input::get_modifiers();
            let msg = format!("Shift: {} | Ctrl: {} | Alt: {} | CapsLock: {}", mods.shift, mods.ctrl, mods.alt, mods.caps_lock);
            serial_println!("{}", msg);
            println!("{}", msg);
        }
        "uname" => {
            serial_println!("open_kernel 0.1.0 x86_64 bare-metal");
            println!("open_kernel 0.1.0 x86_64 bare-metal");
        }
        "sysinfo" => {
            let ticks = scheduler::uptime_ticks();
            let (used, free) = crate::allocator::heap_stats();
            let sysinfo_text = format!("\
    ___   _  __   root@open_kernel
   / _ \\ | |/ /   ----------------
  | | | || ' /    OS: open_kernel 0.1.0 x86_64
  | |_| || . \\    Kernel: 0.1.0-baremetal
   \\___/ |_|\\_\\   Uptime: {} ticks
                  Heap: {} / {} B ({} free)
                  Shell: custom (vfs enabled)", ticks, used, crate::allocator::HEAP_SIZE, free);
            serial_println!("{}", sysinfo_text);
            println!("{}", sysinfo_text);
        }
        "meminfo" => {
            let (used, free) = crate::allocator::heap_stats();
            let total = crate::allocator::HEAP_SIZE;
            let msg = format!("Heap Usage: {} / {} bytes ({:.1}% used, {} bytes free)", 
                used, total, (used as f32 / total as f32) * 100.0, free);
            serial_println!("{}", msg);
            println!("{}", msg);
        }
        "uptime" => {
            let ticks = scheduler::uptime_ticks();
            let total_secs = ticks / 20;
            let hours = total_secs / 3600;
            let mins = (total_secs % 3600) / 60;
            let secs = total_secs % 60;
            let msg = format!("uptime: {:02}:{:02}:{:02} ({} ticks)", hours, mins, secs, ticks);
            serial_println!("{}", msg);
            println!("{}", msg);
        }
        "cd" => {
            let target = if args.is_empty() { "/" } else { args };
            if crate::vfs::change_directory(target) {
                let current = crate::vfs::get_current_directory();
                serial_println!("changed directory to {}", current);
                println!("changed directory to {}", current);
            } else {
                serial_println!("cd: no such directory: {}", args);
                println!("cd: no such directory: {}", args);
            }
        }
        "pwd" => {
            let current = crate::vfs::get_current_directory();
            serial_println!("{}", current);
            println!("{}", current);
        }
        "prompt" => {
            if !args.is_empty() {
                let mut p = PROMPT.lock();
                p.clear();
                p.push_str(args);
                if !args.ends_with(' ') && !args.ends_with('>') {
                    p.push_str("> ");
                }
                let msg = format!("Prompt updated to '{}'", p);
                serial_println!("{}", msg);
                println!("{}", msg);
            } else {
                serial_println!("usage: prompt <new_prompt>");
                println!("usage: prompt <new_prompt>");
            }
        }
        "matrix" => {
            vga_buffer::set_colors(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
            let matrix_art = "\
 0 1 0 1 1 0 1 0 0 1 0 1 1 0 1
 1 0 1 0 0 1 0 1 1 0 1 0 0 1 0
 0 1 1 0 1 0 1 0 0 1 0 1 1 0 1
 MATRIX DIGITAL RAIN INITIALIZED";
            serial_println!("{}", matrix_art);
            println!("{}", matrix_art);
        }
        "guess" => {
            let time = crate::rtc::read_rtc();
            let target = ((time.second as usize % 10) + 1) as i64;
            if let Ok(val) = args.parse::<i64>() {
                if val == target {
                    serial_println!("Correct! You guessed the secret number {}!", target);
                    println!("Correct! You guessed the secret number {}!", target);
                } else if val < target {
                    serial_println!("Too low! Try again.");
                    println!("Too low! Try again.");
                } else {
                    serial_println!("Too high! Try again.");
                    println!("Too high! Try again.");
                }
            } else {
                serial_println!("Guess a number between 1 and 10! usage: guess <number>");
                println!("Guess a number between 1 and 10! usage: guess <number>");
            }
        }
        "time" => {
            let time = crate::rtc::read_rtc();
            serial_println!("time: {:02}:{:02}:{:02}", time.hour, time.minute, time.second);
            println!("time: {:02}:{:02}:{:02}", time.hour, time.minute, time.second);
        }
        "date" => {
            let time = crate::rtc::read_rtc();
            serial_println!("date: {:04}-{:02}-{:02}", time.year, time.month, time.day);
            println!("date: {:04}-{:02}-{:02}", time.year, time.month, time.day);
        }
        "ls" => {
            crate::vfs::list_directory(args);
        }
        "tree" => {
            crate::vfs::print_tree();
        }
        "cat" => {
            if args.is_empty() {
                serial_println!("usage: cat <filename>");
                println!("usage: cat <filename>");
            } else if let Some(content) = crate::vfs::read_file_string(args) {
                serial_println!("{}", content);
                println!("{}", content);
            } else {
                serial_println!("cat: file not found: {}", args);
                println!("cat: file not found: {}", args);
            }
        }
        "touch" => {
            if args.is_empty() {
                serial_println!("usage: touch <filename>");
                println!("usage: touch <filename>");
            } else if crate::vfs::write_file_string(args, "") {
                serial_println!("created empty file '{}'", args);
                println!("created empty file '{}'", args);
            } else {
                serial_println!("touch: failed to create file: {}", args);
                println!("touch: failed to create file: {}", args);
            }
        }
        "write" => {
            let (path, content) = match args.find(' ') {
                Some(idx) => (&args[..idx], args[idx + 1..].trim()),
                None => (args, ""),
            };
            if path.is_empty() {
                serial_println!("usage: write <filename> <text>");
                println!("usage: write <filename> <text>");
            } else if crate::vfs::write_file_string(path, content) {
                serial_println!("wrote to file '{}'", path);
                println!("wrote to file '{}'", path);
            } else {
                serial_println!("write: failed to write file: {}", path);
                println!("write: failed to write file: {}", path);
            }
        }
        "mkdir" => {
            if args.is_empty() {
                serial_println!("usage: mkdir <dirname>");
                println!("usage: mkdir <dirname>");
            } else if crate::vfs::create_dir(args) {
                serial_println!("created directory '{}'", args);
                println!("created directory '{}'", args);
            } else {
                serial_println!("mkdir: failed to create directory: {}", args);
                println!("mkdir: failed to create directory: {}", args);
            }
        }
        "rm" => {
            if args.is_empty() {
                serial_println!("usage: rm <path>");
                println!("usage: rm <path>");
            } else if crate::vfs::remove_node(args) {
                serial_println!("removed '{}'", args);
                println!("removed '{}'", args);
            } else {
                serial_println!("rm: failed to remove: {}", args);
                println!("rm: failed to remove: {}", args);
            }
        }
        "echo" => {
            serial_println!("{}", args);
            println!("{}", args);
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
        "cpuid" => {
            let vendor = get_cpuid_vendor();
            let msg = format!("CPU Vendor: {}\nArch: x86_64 64-bit Mode\nFeatures: FPU PSE APIC MSR PAE PGE", vendor);
            serial_println!("{}", msg);
            println!("{}", msg);
        }
        "color" => {
            match args {
                "matrix" | "green" => {
                    vga_buffer::set_colors(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
                    serial_println!("Theme set to Matrix Green");
                    println!("Theme set to Matrix Green");
                }
                "amber" => {
                    vga_buffer::set_colors(vga_buffer::Color::Amber, vga_buffer::Color::Black);
                    serial_println!("Theme set to Amber");
                    println!("Theme set to Amber");
                }
                "cyberpunk" | "cyan" => {
                    vga_buffer::set_colors(vga_buffer::Color::Cyan, vga_buffer::Color::Black);
                    serial_println!("Theme set to Cyberpunk Cyan");
                    println!("Theme set to Cyberpunk Cyan");
                }
                _ => {
                    vga_buffer::set_colors(vga_buffer::Color::White, vga_buffer::Color::Black);
                    serial_println!("Theme set to Default White");
                    println!("Theme set to Default White");
                }
            }
        }
        "spawn" => {
            let name = if args.is_empty() { "user-task" } else { args };
            if let Some(id) = scheduler::spawn_task("user-worker") {
                let msg = format!("Spawned task '{}' with pid {}", name, id);
                serial_println!("{}", msg);
                println!("{}", msg);
            } else {
                serial_println!("Failed to spawn task: scheduler full");
                println!("Failed to spawn task: scheduler full");
            }
        }
        "kill" => {
            if let Ok(pid) = args.parse::<usize>() {
                if scheduler::kill_task(pid) {
                    let msg = format!("Killed task pid {}", pid);
                    serial_println!("{}", msg);
                    println!("{}", msg);
                } else {
                    let msg = format!("Failed to kill pid {}: invalid pid or finished", pid);
                    serial_println!("{}", msg);
                    println!("{}", msg);
                }
            } else {
                serial_println!("usage: kill <pid>");
                println!("usage: kill <pid>");
            }
        }
        "calc" => {
            let parts: Vec<&str> = args.split_whitespace().collect();
            if parts.len() == 3 {
                let a = parts[0].parse::<i64>();
                let op = parts[1];
                let b = parts[2].parse::<i64>();
                if let (Ok(num1), Ok(num2)) = (a, b) {
                    let res = match op {
                        "+" => Some(num1 + num2),
                        "-" => Some(num1 - num2),
                        "*" => Some(num1 * num2),
                        "/" if num2 != 0 => Some(num1 / num2),
                        "%" if num2 != 0 => Some(num1 % num2),
                        _ => None,
                    };
                    if let Some(val) = res {
                        let msg = format!("{} {} {} = {}", num1, op, num2, val);
                        serial_println!("{}", msg);
                        println!("{}", msg);
                    } else {
                        serial_println!("calc error: invalid operator or division by zero");
                        println!("calc error: invalid operator or division by zero");
                    }
                } else {
                    serial_println!("usage: calc <num1> <+|-|*|/|%> <num2>");
                    println!("usage: calc <num1> <+|-|*|/|%> <num2>");
                }
            } else {
                serial_println!("usage: calc <num1> <+|-|*|/|%> <num2>");
                println!("usage: calc <num1> <+|-|*|/|%> <num2>");
            }
        }
        "panic" => {
            panic!("Kernel panic triggered via shell");
        }
        "reboot" => {
            serial_println!("rebooting system...");
            println!("rebooting system...");
            unsafe {
                x86_64::instructions::port::Port::<u8>::new(0x64).write(0xFE);
            }
        }
        "shutdown" => {
            serial_println!("shutting down system...");
            println!("shutting down system...");
            unsafe {
                x86_64::instructions::port::Port::<u16>::new(0x604).write(0x2000);
                x86_64::instructions::port::Port::<u16>::new(0x4004).write(0x3400);
                x86_64::instructions::port::Port::<u16>::new(0x600).write(0x3400);
            }
        }
        other => {
            serial_println!("unknown command: {}", other);
            println!("unknown command: {}", other);
        }
    }
}

fn get_cpuid_vendor() -> &'static str {
    let res = core::arch::x86_64::__cpuid(0);
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&res.ebx.to_le_bytes());
    b[4..8].copy_from_slice(&res.edx.to_le_bytes());
    b[8..12].copy_from_slice(&res.ecx.to_le_bytes());
    match &b {
        b"GenuineIntel" => "GenuineIntel (Intel CPU)",
        b"AuthenticAMD" => "AuthenticAMD (AMD CPU)",
        b"TCGTCGTCGTCG" => "TCG (QEMU Virtual CPU)",
        _ => "x86_64 Compatible CPU",
    }
}
