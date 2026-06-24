#![no_main]
#![no_std]

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi_services::println;
use bootloader_api::{BootInfo, MemoryMap, MemoryRegion, MemoryRegionRange, MemoryRegionType};

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
    let file = match root_dir.open(cstr16!("kernel.elf"), FileMode::Read, FileAttribute::empty()) {
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
    use uefi::table::boot::{AllocateType, MemoryType};
    use core::slice;

    println!("Found kernel.elf! Reading file size...");

    let mut info_buf = vec![0; 1024];
    let info = file.get_info::<FileInfo>(&mut info_buf).unwrap();
    let file_size = info.file_size() as usize;

    let mut elf_buf = vec::Vec::with_capacity(file_size);
    elf_buf.resize(file_size, 0);
    file.read(&mut elf_buf).unwrap();

    println!("Parsing ELF...");
    let elf_file = xmas_elf::ElfFile::new(&elf_buf).expect("Failed to parse ELF");

    println!("Loading ELF segments...");
    for ph in elf_file.program_iter() {
        if let xmas_elf::program::Type::Load = ph.get_type().unwrap() {
            let p_paddr = ph.physical_addr();
            let p_filesz = ph.file_size();
            let p_memsz = ph.mem_size();

            if p_memsz == 0 {
                continue;
            }

            let pages = (p_memsz + 0xFFF) / 0x1000;

            let alloc_addr = boot_services.allocate_pages(
                AllocateType::Address(p_paddr as u64),
                MemoryType::LOADER_DATA,
                pages as usize
            ).expect("Failed to allocate pages for kernel segment");

            let offset = ph.offset() as usize;
            let src = &elf_buf[offset .. offset + p_filesz as usize];
            unsafe {
                let dst = slice::from_raw_parts_mut(alloc_addr as *mut u8, p_filesz as usize);
                dst.copy_from_slice(src);

                if p_memsz > p_filesz {
                    let zero_dst = slice::from_raw_parts_mut(
                        (alloc_addr + p_filesz) as *mut u8,
                        (p_memsz - p_filesz) as usize
                    );
                    for b in zero_dst {
                        *b = 0;
                    }
                }
            }
        }
    }

    let entry_point = elf_file.header.pt2.entry_point();

    // Drop file system handles to release the borrow on boot_services
    drop(file);
    drop(root_dir);
    drop(sfs);

    println!("Exiting boot services and jumping to kernel at {:#x}...", entry_point);

    let (_system_table, memory_map) = system_table.exit_boot_services();

    // Populate BootInfo
    static mut BOOT_INFO: BootInfo = BootInfo {
        memory_map: MemoryMap::new(),
        physical_memory_offset: 0, // In a real scenario, this is where we map all physical memory
    };

    unsafe {
        let boot_info_ptr = core::ptr::addr_of_mut!(BOOT_INFO);
        let boot_info = &mut *boot_info_ptr;

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
        let kernel_entry: extern "sysv64" fn(&'static mut BootInfo) -> ! = core::mem::transmute(entry_point as usize);
        kernel_entry(boot_info);
    }
}

