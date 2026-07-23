use alloc::{string::String, string::ToString, vec::Vec};
use lazy_static::lazy_static;
use spin::Mutex;

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn device_type(&self) -> &str;
    fn init(&mut self) -> bool;
    fn status(&self) -> &str;
}

pub trait BlockDevice: Driver {
    fn block_size(&self) -> usize;
    fn block_count(&self) -> usize;
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool;
    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> bool;
}

pub trait DisplayDriver: Driver {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
}

pub struct RamDiskDriver {
    name: String,
    blocks: Vec<Vec<u8>>,
    block_size: usize,
    status: String,
}

impl RamDiskDriver {
    pub fn new(block_count: usize, block_size: usize) -> Self {
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            blocks.push(alloc::vec![0u8; block_size]);
        }
        Self {
            name: String::from("ramdisk0"),
            blocks,
            block_size,
            status: String::from("active"),
        }
    }
}

impl Driver for RamDiskDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> &str {
        "Storage (Block Device)"
    }

    fn init(&mut self) -> bool {
        self.status = String::from("ready");
        true
    }

    fn status(&self) -> &str {
        &self.status
    }
}

impl BlockDevice for RamDiskDriver {
    fn block_size(&self) -> usize {
        self.block_size
    }

    fn block_count(&self) -> usize {
        self.blocks.len()
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool {
        if block_id < self.blocks.len() && buf.len() >= self.block_size {
            buf[..self.block_size].copy_from_slice(&self.blocks[block_id]);
            true
        } else {
            false
        }
    }

    fn write_block(&mut self, block_id: usize, buf: &[u8]) -> bool {
        if block_id < self.blocks.len() && buf.len() >= self.block_size {
            self.blocks[block_id].copy_from_slice(&buf[..self.block_size]);
            true
        } else {
            false
        }
    }
}

pub struct FramebufferDisplayDriver {
    name: String,
    status: String,
}

impl FramebufferDisplayDriver {
    pub fn new() -> Self {
        Self {
            name: String::from("gop-graphics"),
            status: String::from("ready"),
        }
    }
}

impl Driver for FramebufferDisplayDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> &str {
        "Graphics (GOP Framebuffer)"
    }

    fn init(&mut self) -> bool {
        self.status = String::from("active");
        true
    }

    fn status(&self) -> &str {
        &self.status
    }
}

impl DisplayDriver for FramebufferDisplayDriver {
    fn width(&self) -> usize {
        1024
    }

    fn height(&self) -> usize {
        768
    }
}

pub struct DriverManager {
    drivers: Vec<alloc::boxed::Box<dyn Driver>>,
}

impl DriverManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            drivers: Vec::new(),
        };
        mgr.register(alloc::boxed::Box::new(RamDiskDriver::new(128, 512)));
        mgr.register(alloc::boxed::Box::new(FramebufferDisplayDriver::new()));
        mgr
    }

    pub fn register(&mut self, driver: alloc::boxed::Box<dyn Driver>) {
        self.drivers.push(driver);
    }

    pub fn list_drivers(&self) -> Vec<(String, String, String)> {
        self.drivers
            .iter()
            .map(|d| (d.name().to_string(), d.device_type().to_string(), d.status().to_string()))
            .collect()
    }
}

lazy_static! {
    pub static ref DRIVERS: Mutex<DriverManager> = Mutex::new(DriverManager::new());
}

pub fn print_drivers() {
    let mgr = DRIVERS.lock();
    let list = mgr.list_drivers();
    crate::serial_println!("Registered Drivers ({}):", list.len());
    crate::println!("Registered Drivers ({}):", list.len());
    for (name, dev_type, status) in list {
        let msg = alloc::format!("  {:<14} [{:<24}] Status: {}", name, dev_type, status);
        crate::serial_println!("{}", msg);
        crate::println!("{}", msg);
    }
}
