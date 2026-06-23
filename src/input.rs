use alloc::collections::VecDeque;

use lazy_static::lazy_static;
use spin::Mutex;

const MAX_LINE_LEN: usize = 128;
const MAX_QUEUE_DEPTH: usize = 8;

pub struct InputBuffer {
    line: VecDeque<char>,
    ready_lines: VecDeque<alloc::string::String>,
}

impl InputBuffer {
    fn new() -> Self {
        Self {
            line: VecDeque::new(),
            ready_lines: VecDeque::new(),
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

    pub fn commit_line(&mut self) {
        let mut line = alloc::string::String::new();
        while let Some(ch) = self.line.pop_front() {
            line.push(ch);
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
    pub static ref INPUT: Mutex<InputBuffer> = Mutex::new(InputBuffer::new());
}
