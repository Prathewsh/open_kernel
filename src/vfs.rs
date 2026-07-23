use alloc::{string::String, string::ToString, vec::Vec};
use lazy_static::lazy_static;
use spin::Mutex;

#[derive(Debug, Clone)]
pub enum VfsNode {
    File {
        name: String,
        content: Vec<u8>,
    },
    Directory {
        name: String,
        children: Vec<VfsNode>,
    },
}

impl VfsNode {
    pub fn name(&self) -> &str {
        match self {
            VfsNode::File { name, .. } => name,
            VfsNode::Directory { name, .. } => name,
        }
    }
}

pub struct FileSystem {
    root: VfsNode,
}

impl FileSystem {
    pub fn new() -> Self {
        let mut fs = Self {
            root: VfsNode::Directory {
                name: String::from("/"),
                children: Vec::new(),
            },
        };
        fs.init_default_structure();
        fs
    }

    fn init_default_structure(&mut self) {
        self.mkdir("config");
        self.mkdir("bin");
        self.mkdir("home");
        self.write_file(
            "config/system.conf",
            b"[system]\nos_name = open_kernel\nversion = 0.1.0\narch = x86_64\n",
        );
        self.write_file(
            "readme.txt",
            b"Welcome to open_kernel OS in-memory VFS!\n",
        );
        self.write_file(
            "bin/init",
            b"#!/bin/sh\necho booting open_kernel...\n",
        );
    }

    fn find_node_mut<'a>(&'a mut self, path: &str) -> Option<&'a mut VfsNode> {
        let clean_path = path.trim_matches('/');
        if clean_path.is_empty() {
            return Some(&mut self.root);
        }
        let parts: Vec<&str> = clean_path.split('/').filter(|s| !s.is_empty()).collect();
        let mut curr = &mut self.root;

        for part in parts {
            match curr {
                VfsNode::Directory { children, .. } => {
                    let mut found_idx = None;
                    for (i, child) in children.iter().enumerate() {
                        if child.name() == part {
                            found_idx = Some(i);
                            break;
                        }
                    }
                    if let Some(idx) = found_idx {
                        curr = &mut children[idx];
                    } else {
                        return None;
                    }
                }
                VfsNode::File { .. } => return None,
            }
        }
        Some(curr)
    }

    pub fn list_directory(&mut self, path: &str) -> Option<Vec<(String, bool)>> {
        let node = self.find_node_mut(path)?;
        match node {
            VfsNode::Directory { children, .. } => {
                let mut res = Vec::new();
                for child in children {
                    let is_dir = matches!(child, VfsNode::Directory { .. });
                    res.push((child.name().to_string(), is_dir));
                }
                Some(res)
            }
            _ => None,
        }
    }

    pub fn read_file(&mut self, path: &str) -> Option<Vec<u8>> {
        let node = self.find_node_mut(path)?;
        match node {
            VfsNode::File { content, .. } => Some(content.clone()),
            _ => None,
        }
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> bool {
        let clean_path = path.trim_matches('/');
        if clean_path.is_empty() {
            return false;
        }
        let (parent_path, file_name) = match clean_path.rfind('/') {
            Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
            None => ("", clean_path),
        };

        if let Some(parent) = self.find_node_mut(parent_path) {
            if let VfsNode::Directory { children, .. } = parent {
                for child in children.iter_mut() {
                    if child.name() == file_name {
                        if let VfsNode::File { content, .. } = child {
                            content.clear();
                            content.extend_from_slice(data);
                            return true;
                        }
                        return false;
                    }
                }
                children.push(VfsNode::File {
                    name: file_name.to_string(),
                    content: data.to_vec(),
                });
                return true;
            }
        }
        false
    }

    pub fn mkdir(&mut self, path: &str) -> bool {
        let clean_path = path.trim_matches('/');
        if clean_path.is_empty() {
            return false;
        }
        let (parent_path, dir_name) = match clean_path.rfind('/') {
            Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
            None => ("", clean_path),
        };

        if let Some(parent) = self.find_node_mut(parent_path) {
            if let VfsNode::Directory { children, .. } = parent {
                for child in children.iter() {
                    if child.name() == dir_name {
                        return false;
                    }
                }
                children.push(VfsNode::Directory {
                    name: dir_name.to_string(),
                    children: Vec::new(),
                });
                return true;
            }
        }
        false
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let clean_path = path.trim_matches('/');
        if clean_path.is_empty() {
            return false;
        }
        let (parent_path, target_name) = match clean_path.rfind('/') {
            Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
            None => ("", clean_path),
        };

        if let Some(parent) = self.find_node_mut(parent_path) {
            if let VfsNode::Directory { children, .. } = parent {
                let pos = children.iter().position(|c| c.name() == target_name);
                if let Some(idx) = pos {
                    children.remove(idx);
                    return true;
                }
            }
        }
        false
    }
}

lazy_static! {
    pub static ref FS: Mutex<FileSystem> = Mutex::new(FileSystem::new());
    pub static ref CURRENT_DIR: Mutex<String> = Mutex::new(String::from("/"));
}

pub fn get_current_directory() -> String {
    CURRENT_DIR.lock().clone()
}

pub fn resolve_path(input_path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let base = if input_path.starts_with('/') {
        String::from("/")
    } else {
        CURRENT_DIR.lock().clone()
    };

    for segment in base.split('/').chain(input_path.split('/')) {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            sub => {
                parts.push(sub.to_string());
            }
        }
    }

    if parts.is_empty() {
        String::from("/")
    } else {
        let mut res = String::from("/");
        res.push_str(&parts.join("/"));
        res
    }
}

pub fn change_directory(path: &str) -> bool {
    let resolved = resolve_path(path);
    let mut fs = FS.lock();
    if resolved == "/" {
        *CURRENT_DIR.lock() = String::from("/");
        return true;
    }
    if let Some(node) = fs.find_node_mut(&resolved) {
        if matches!(node, VfsNode::Directory { .. }) {
            *CURRENT_DIR.lock() = resolved;
            return true;
        }
    }
    false
}

pub fn list_directory(path: &str) {
    let target = if path.is_empty() {
        CURRENT_DIR.lock().clone()
    } else {
        resolve_path(path)
    };
    let mut fs = FS.lock();
    if let Some(entries) = fs.list_directory(&target) {
        crate::serial_println!("Directory listing [{}]:", target);
        crate::println!("Directory listing [{}]:", target);
        if entries.is_empty() {
            crate::serial_println!("  (empty)");
            crate::println!("  (empty)");
        } else {
            for (name, is_dir) in entries {
                let suffix = if is_dir { "/" } else { "" };
                crate::serial_println!("  {}{}", name, suffix);
                crate::println!("  {}{}", name, suffix);
            }
        }
    } else {
        crate::serial_println!("ls: directory not found: {}", target);
        crate::println!("ls: directory not found: {}", target);
    }
}

pub fn print_tree() {
    let mut fs = FS.lock();
    crate::serial_println!(".");
    crate::println!(".");
    if let Some(entries) = fs.list_directory("") {
        for (name, is_dir) in entries {
            let suffix = if is_dir { "/" } else { "" };
            crate::serial_println!("|-- {}{}", name, suffix);
            crate::println!("|-- {}{}", name, suffix);
        }
    }
}

pub fn read_file_string(path: &str) -> Option<String> {
    let resolved = resolve_path(path);
    let mut fs = FS.lock();
    fs.read_file(&resolved).and_then(|bytes| String::from_utf8(bytes).ok())
}

pub fn write_file_string(path: &str, content: &str) -> bool {
    let resolved = resolve_path(path);
    let mut fs = FS.lock();
    fs.write_file(&resolved, content.as_bytes())
}

pub fn create_dir(path: &str) -> bool {
    let resolved = resolve_path(path);
    let mut fs = FS.lock();
    fs.mkdir(&resolved)
}

pub fn remove_node(path: &str) -> bool {
    let resolved = resolve_path(path);
    let mut fs = FS.lock();
    fs.remove(&resolved)
}
                        
