use alloc::{format, string::String, vec::Vec};
use spin::Mutex;
use x86_64::{
    structures::paging::{PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

#[derive(Debug)]
pub struct ProcessAddressSpace {
    pub pid: usize,
    pub name: String,
    pub cr3_frame: PhysFrame<Size4KiB>,
    pub code_range: (VirtAddr, VirtAddr),
    pub stack_range: (VirtAddr, VirtAddr),
    pub heap_range: (VirtAddr, VirtAddr),
    pub isolated: bool,
}

impl ProcessAddressSpace {
    pub fn create_kernel_space() -> Self {
        Self {
            pid: 0,
            name: String::from("kernel"),
            cr3_frame: PhysFrame::containing_address(PhysAddr::new(0x1000)),
            code_range: (VirtAddr::new(0x200000), VirtAddr::new(0x400000)),
            stack_range: (VirtAddr::new(0x800000), VirtAddr::new(0x900000)),
            heap_range: (VirtAddr::new(0x444444440000), VirtAddr::new(0x444444480000)),
            isolated: false,
        }
    }

    pub fn create_user_space(pid: usize, name: &str) -> Self {
        let base = 0x100000000u64 + (pid as u64 * 0x10000000u64);
        Self {
            pid,
            name: String::from(name),
            cr3_frame: PhysFrame::containing_address(PhysAddr::new(0x2000 + (pid as u64 * 0x1000))),
            code_range: (VirtAddr::new(base), VirtAddr::new(base + 0x200000)),
            stack_range: (VirtAddr::new(base + 0x7FF00000), VirtAddr::new(base + 0x80000000)),
            heap_range: (VirtAddr::new(base + 0x400000), VirtAddr::new(base + 0x800000)),
            isolated: true,
        }
    }
}

pub struct MemoryManager {
    spaces: Vec<ProcessAddressSpace>,
}

impl MemoryManager {
    pub fn new() -> Self {
        let mut mm = Self { spaces: Vec::new() };
        mm.spaces.push(ProcessAddressSpace::create_kernel_space());
        mm.spaces.push(ProcessAddressSpace::create_user_space(1, "init"));
        mm.spaces.push(ProcessAddressSpace::create_user_space(2, "shell"));
        mm
    }

    pub fn print_vmap(&self) {
        crate::serial_println!("PID NAME      CR3 FRAME          VIRTUAL MEMORY RANGES                    ISOLATION");
        crate::println!("PID NAME      CR3 FRAME          VIRTUAL MEMORY RANGES                    ISOLATION");
        for space in &self.spaces {
            let msg = format!(
                "{:>3} {:<9} {:#014x} Code: {:#x} Stack: {:#x} {}",
                space.pid,
                space.name,
                space.cr3_frame.start_address().as_u64(),
                space.code_range.0.as_u64(),
                space.stack_range.0.as_u64(),
                if space.isolated { "[ISOLATED USER MAPPING]" } else { "[SHARED KERNEL MAPPING]" }
            );
            crate::serial_println!("{}", msg);
            crate::println!("{}", msg);
        }
    }
}

lazy_static::lazy_static! {
    pub static ref VM_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());
}

pub fn print_vmap() {
    VM_MANAGER.lock().print_vmap();
}
