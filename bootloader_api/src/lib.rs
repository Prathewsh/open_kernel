#![no_std]

/// Represents the boot information passed from the bootloader to the kernel.
#[derive(Debug)]
#[repr(C)]
pub struct BootInfo {
    pub memory_map: MemoryMap,
    pub physical_memory_offset: u64,
    pub framebuffer: FrameBufferInfo,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    pub address: u64,
    pub byte_len: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub pixel_format: PixelFormat,
}

impl FrameBufferInfo {
    pub const fn empty() -> Self {
        Self {
            address: 0,
            byte_len: 0,
            width: 0,
            height: 0,
            stride: 0,
            pixel_format: PixelFormat::Bgr,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}

/// A simple memory map representing usable memory regions.
#[derive(Debug)]
#[repr(C)]
pub struct MemoryMap {
    regions: [MemoryRegion; 256],
    next_free_index: usize,
}

impl MemoryMap {
    pub const fn new() -> Self {
        Self {
            regions: [MemoryRegion::empty(); 256],
            next_free_index: 0,
        }
    }

    pub fn add_region(&mut self, region: MemoryRegion) {
        if self.next_free_index < self.regions.len() {
            self.regions[self.next_free_index] = region;
            self.next_free_index += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.regions[..self.next_free_index].iter()
    }
}

/// A region of physical memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MemoryRegion {
    pub range: MemoryRegionRange,
    pub region_type: MemoryRegionType,
}

impl MemoryRegion {
    pub const fn empty() -> Self {
        Self {
            range: MemoryRegionRange {
                start_addr: 0,
                end_addr: 0,
            },
            region_type: MemoryRegionType::Usable,
        }
    }
}

/// A range of physical addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MemoryRegionRange {
    pub start_addr: u64,
    pub end_addr: u64,
}

impl MemoryRegionRange {
    pub fn start_addr(&self) -> u64 {
        self.start_addr
    }
    pub fn end_addr(&self) -> u64 {
        self.end_addr
    }
}

/// The type of a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum MemoryRegionType {
    Usable,
    InUse,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    Kernel,
    KernelStack,
    PageTable,
    Bootloader,
    UnknownUefi(u32),
}
