pub fn list_directory() {
    let dummy_files = [
        "kernel.elf",
        "config/",
        "bin/",
        "home/",
    ];
    crate::serial_println!("Directory listing:");
    crate::println!("Directory listing:");
    for file in dummy_files.iter() {
        crate::serial_println!("  {}", file);
        crate::println!("  {}", file);
    }
}

pub fn print_tree() {
    let tree_output = "\
.
|-- bin/
|   |-- shell
|   `-- init
|-- config/
|   `-- system.conf
|-- home/
|   `-- user/
`-- kernel.elf";
    crate::serial_println!("{}", tree_output);
    crate::println!("{}", tree_output);
}                        
