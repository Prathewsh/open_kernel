use bootloader_api::{MemoryMap, MemoryRegionType};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB,
    },
};

/// Build an OffsetPageTable that maps every physical address at `physical_memory_offset`.
///
/// SAFETY: `physical_memory_offset` must be the correct offset at which the
/// bootloader mapped all physical memory, and this function must be called
/// at most once.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let l4 = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(l4, physical_memory_offset)
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;
    let (frame, _) = Cr3::read();
    let virt = physical_memory_offset + frame.start_address().as_u64();
    &mut *virt.as_mut_ptr::<PageTable>()
}

/// Allocates 4 KiB physical frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// SAFETY: caller guarantees the memory map is valid and that usable
    /// frames are not already in use by anything else.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator { memory_map, next: 0 }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        self.memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .flat_map(|r| (r.range.start_addr()..r.range.end_addr()).step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
