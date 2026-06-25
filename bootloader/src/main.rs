#![no_main]
#![no_std]

use bootloader_api::{
    BootInfo, FrameBufferInfo, MemoryMap, MemoryRegion, MemoryRegionRange, MemoryRegionType,
    PixelFormat as BootPixelFormat,
};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi_services::println;

#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    println!("Welcome to the Custom UEFI Bootloader!");

    // Get the boot services instance
    let boot_services = system_table.boot_services();

    println!("Attempting to locate the file system protocol...");

    // Open the simple file system protocol on the device the bootloader was loaded from
    let sfs_handle = boot_services
        .get_handle_for_protocol::<SimpleFileSystem>()
        .unwrap();

    let mut sfs = boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(sfs_handle)
        .unwrap();

    // Open the root directory
    let mut root_dir = sfs.open_volume().unwrap();

    println!("Attempting to open the kernel file...");
    // Attempt to open the kernel file (assuming it's named 'kernel.elf' in the root)
    let file = match root_dir.open(
        cstr16!("kernel.elf"),
        FileMode::Read,
        FileAttribute::empty(),
    ) {
        Ok(f) => f,
        Err(_) => {
            println!("Failed to find kernel.elf. Ensure the kernel is present in the EFI partition root.");
            return Status::NOT_FOUND;
        }
    };

    let mut file = match file.into_type().unwrap() {
        uefi::proto::media::file::FileType::Regular(f) => f,
        _ => {
            println!("kernel.elf is not a regular file!");
            return Status::INVALID_PARAMETER;
        }
    };

    extern crate alloc;
    use alloc::vec;
    use core::slice;
    use uefi::table::boot::{AllocateType, MemoryType};

    println!("Found kernel.elf! Reading file size...");

    let mut info_buf = vec![0; 1024];
    let info = file.get_info::<FileInfo>(&mut info_buf).unwrap();
    let file_size = info.file_size() as usize;

    let mut elf_buf = vec![0; file_size];
    let bytes_read = file.read(&mut elf_buf).unwrap();
    if bytes_read != file_size {
        println!(
            "Failed to read kernel.elf completely: expected {} bytes, got {}",
            file_size, bytes_read
        );
        return Status::DEVICE_ERROR;
    }

    println!("Parsing ELF...");
    let elf_file = xmas_elf::ElfFile::new(&elf_buf).expect("Failed to parse ELF");

    println!("Loading ELF segments...");

    // First pass: find the total physical address range across all LOAD segments
    let mut load_min: u64 = u64::MAX;
    let mut load_max: u64 = 0;
    for ph in elf_file.program_iter() {
        if let xmas_elf::program::Type::Load = ph.get_type().unwrap() {
            let start = ph.physical_addr();
            let end = start + ph.mem_size();
            if start < load_min {
                load_min = start;
            }
            if end > load_max {
                load_max = end;
            }
        }
    }

    if load_min >= load_max {
        println!("No LOAD segments found!");
        return Status::NOT_FOUND;
    }

    // Page-align: round down start, round up end
    let alloc_start = load_min & !0xFFF;
    let alloc_end = (load_max + 0xFFF) & !0xFFF;
    let total_pages = ((alloc_end - alloc_start) / 0x1000) as usize;

    println!(
        "  Kernel range: {:#x} - {:#x} ({} pages)",
        alloc_start, alloc_end, total_pages
    );

    let alloc_addr = boot_services
        .allocate_pages(
            AllocateType::Address(alloc_start),
            MemoryType::LOADER_DATA,
            total_pages,
        )
        .expect("Failed to allocate pages for kernel");

    // Zero the entire allocated region
    unsafe {
        core::ptr::write_bytes(alloc_addr as *mut u8, 0, total_pages * 0x1000);
    }

    // Second pass: copy each LOAD segment's file data into the allocated region
    for ph in elf_file.program_iter() {
        if let xmas_elf::program::Type::Load = ph.get_type().unwrap() {
            let p_paddr = ph.physical_addr();
            let p_filesz = ph.file_size() as usize;
            let p_memsz = ph.mem_size();

            if p_memsz == 0 {
                continue;
            }

            println!(
                "  Segment: paddr={:#x}, filesz={:#x}, memsz={:#x}",
                p_paddr, p_filesz, p_memsz
            );

            let offset = ph.offset() as usize;
            let src = &elf_buf[offset..offset + p_filesz];
            unsafe {
                let dst = slice::from_raw_parts_mut(p_paddr as *mut u8, p_filesz);
                dst.copy_from_slice(src);
            }
        }
    }

    let entry_point = elf_file.header.pt2.entry_point();

    // Drop file system handles to release the borrow on boot_services
    drop(file);
    drop(root_dir);
    drop(sfs);

    let gop_handle = boot_services
        .get_handle_for_protocol::<GraphicsOutput>()
        .expect("Graphics Output Protocol not found");
    let mut gop = boot_services
        .open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Failed to open Graphics Output Protocol");
    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let pixel_format = match mode_info.pixel_format() {
        PixelFormat::Rgb => BootPixelFormat::Rgb,
        PixelFormat::Bgr => BootPixelFormat::Bgr,
        other => panic!("Unsupported GOP pixel format: {:?}", other),
    };
    let framebuffer_info = {
        let mut framebuffer = gop.frame_buffer();
        FrameBufferInfo {
            address: framebuffer.as_mut_ptr() as u64,
            byte_len: framebuffer.size(),
            width,
            height,
            stride: mode_info.stride(),
            pixel_format,
        }
    };
    drop(gop);

    println!(
        "Exiting boot services and jumping to kernel at {:#x}...",
        entry_point
    );

    let (_system_table, memory_map) = system_table.exit_boot_services();

    // Populate BootInfo
    static mut BOOT_INFO: BootInfo = BootInfo {
        memory_map: MemoryMap::new(),
        physical_memory_offset: 0, // In a real scenario, this is where we map all physical memory
        framebuffer: FrameBufferInfo::empty(),
    };

    unsafe {
        let boot_info_ptr = core::ptr::addr_of_mut!(BOOT_INFO);
        let boot_info = &mut *boot_info_ptr;
        boot_info.framebuffer = framebuffer_info;

        for desc in memory_map.entries() {
            let region_type = match desc.ty {
                uefi::table::boot::MemoryType::CONVENTIONAL => MemoryRegionType::Usable,
                _ => MemoryRegionType::Reserved,
            };
            boot_info.memory_map.add_region(MemoryRegion {
                range: MemoryRegionRange {
                    start_addr: desc.phys_start,
                    end_addr: desc.phys_start + desc.page_count * 4096,
                },
                region_type,
            });
        }

        // Jump to the kernel entry point
        let kernel_entry: extern "sysv64" fn(&'static mut BootInfo) -> ! =
            core::mem::transmute(entry_point as usize);
        kernel_entry(boot_info);
    }
}
