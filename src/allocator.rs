use linked_list_allocator::LockedHeap;
use x86_64::{
    VirtAddr,
    structures::paging::{mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
};

// Place the kernel heap well above any bootloader / kernel image addresses.
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE:  usize = 256 * 1024; // 256 KiB — plenty to start

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Map `HEAP_SIZE` bytes of physical frames into the heap virtual range and
/// hand them to the allocator. Call once, after the mapper and frame allocator
/// are ready.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start = VirtAddr::new(HEAP_START as u64);
    let heap_end   = heap_start + HEAP_SIZE as u64 - 1u64;
    let start_page = Page::containing_address(heap_start);
    let end_page   = Page::containing_address(heap_end);

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe { ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE) }

    Ok(())
}
