use alloc::collections::VecDeque;

use lazy_static::lazy_static;
use spin::Mutex;

const MAX_LINE_LEN: usize = 128;
const MAX_QUEUE_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub caps_lock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    Home,
    End,
    Escape,
    FunctionKey(u8),
}

pub struct InputBuffer {
    line: VecDeque<char>,
    ready_lines: VecDeque<alloc::string::String>,
    history: VecDeque<alloc::string::String>,
    modifiers: KeyboardModifiers,
}

impl InputBuffer {
    fn new() -> Self {
        Self {
            line: VecDeque::new(),
            ready_lines: VecDeque::new(),
            history: VecDeque::new(),
            modifiers: KeyboardModifiers::default(),
        }
    }

    pub fn push_char(&mut self, ch: char) {
        if self.line.len() < MAX_LINE_LEN {
            self.line.push_back(ch);
        }
    }

    pub fn backspace(&mut self) -> bool {
        self.line.pop_back().is_some()
    }

    pub fn handle_special_key(&mut self, key: SpecialKey) {
        match key {
            SpecialKey::UpArrow => {
                if let Some(prev) = self.history.back() {
                    self.line.clear();
                    for ch in prev.chars() {
                        self.line.push_back(ch);
                    }
                }
            }
            SpecialKey::Escape => {
                self.line.clear();
            }
            _ => {}
        }
    }

    pub fn set_modifier(&mut self, mod_type: &str, state: bool) {
        match mod_type {
            "shift" => self.modifiers.shift = state,
            "ctrl" => self.modifiers.ctrl = state,
            "alt" => self.modifiers.alt = state,
            "caps" => self.modifiers.caps_lock = state,
            _ => {}
        }
    }

    pub fn commit_line(&mut self) {
        let mut line = alloc::string::String::new();
        while let Some(ch) = self.line.pop_front() {
            line.push(ch);
        }

        if !line.is_empty() {
            if self.history.len() >= 16 {
                self.history.pop_front();
            }
            self.history.push_back(line.clone());
        }

        if self.ready_lines.len() >= MAX_QUEUE_DEPTH {
            self.ready_lines.pop_front();
        }
        self.ready_lines.push_back(line);
    }

    pub fn pop_line(&mut self) -> Option<alloc::string::String> {
        self.ready_lines.pop_front()
    }

    pub fn current_line(&self) -> alloc::string::String {
        self.line.iter().copied().collect()
    }
}

lazy_static! {
    static ref INPUT: Mutex<InputBuffer> = Mutex::new(InputBuffer::new());
}

pub fn push_char(ch: char) {
    x86_64::instructions::interrupts::without_interrupts(|| INPUT.lock().push_char(ch));
}

pub fn backspace() -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| INPUT.lock().backspace())
}

pub fn commit_line() {
    x86_64::instructions::interrupts::without_interrupts(|| INPUT.lock().commit_line());
}

pub fn pop_line() -> Option<alloc::string::String> {
    x86_64::instructions::interrupts::without_interrupts(|| INPUT.lock().pop_line())
}

pub fn get_modifiers() -> KeyboardModifiers {
    x86_64::instructions::interrupts::without_interrupts(|| INPUT.lock().modifiers)
}
