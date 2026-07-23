use alloc::{format, string::String, string::ToString, vec::Vec};
use lazy_static::lazy_static;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Addr(pub [u8; 4]);

impl core::fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

pub struct NetworkInterface {
    pub name: String,
    pub mac: MacAddress,
    pub ip: Ipv4Addr,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

impl NetworkInterface {
    pub fn loopback() -> Self {
        Self {
            name: String::from("lo0"),
            mac: MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            ip: Ipv4Addr([127, 0, 0, 1]),
            rx_packets: 4,
            tx_packets: 4,
            rx_bytes: 256,
            tx_bytes: 256,
        }
    }

    pub fn eth0() -> Self {
        Self {
            name: String::from("eth0"),
            mac: MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            ip: Ipv4Addr([192, 168, 1, 100]),
            rx_packets: 12,
            tx_packets: 8,
            rx_bytes: 1420,
            tx_bytes: 980,
        }
    }
}

pub struct NetworkStack {
    interfaces: Vec<NetworkInterface>,
}

impl NetworkStack {
    pub fn new() -> Self {
        Self {
            interfaces: alloc::vec![NetworkInterface::loopback(), NetworkInterface::eth0()],
        }
    }

    pub fn ping(&mut self, target_ip: &str) -> bool {
        for iface in &mut self.interfaces {
            if iface.ip.to_string() == target_ip || target_ip == "127.0.0.1" || target_ip == "localhost" {
                iface.tx_packets += 4;
                iface.rx_packets += 4;
                iface.tx_bytes += 256;
                iface.rx_bytes += 256;
                return true;
            }
        }
        false
    }
}

lazy_static! {
    pub static ref NET_STACK: Mutex<NetworkStack> = Mutex::new(NetworkStack::new());
}

pub fn print_ifconfig() {
    let stack = NET_STACK.lock();
    for iface in &stack.interfaces {
        let info = format!(
            "{}: flags=UP LOOPBACK RUNNING  mtu 1500\n        inet {}  netmask 255.255.255.0\n        ether {}\n        RX packets {}  bytes {}\n        TX packets {}  bytes {}",
            iface.name, iface.ip, iface.mac, iface.rx_packets, iface.rx_bytes, iface.tx_packets, iface.tx_bytes
        );
        crate::serial_println!("{}", info);
        crate::println!("{}", info);
    }
}

pub fn ping(target: &str) {
    let mut stack = NET_STACK.lock();
    let header = format!("PING {} ({}): 56 data bytes", target, target);
    crate::serial_println!("{}", header);
    crate::println!("{}", header);
    if stack.ping(target) {
        for seq in 1..=4 {
            let line = format!("64 bytes from {}: icmp_seq={} ttl=64 time=0.342 ms", target, seq);
            crate::serial_println!("{}", line);
            crate::println!("{}", line);
        }
        let summary = format!("--- {} ping statistics ---\n4 packets transmitted, 4 packets received, 0.0% packet loss", target);
        crate::serial_println!("{}", summary);
        crate::println!("{}", summary);
    } else {
        let line = format!("Request timeout for icmp_seq 1 to {}", target);
        crate::serial_println!("{}", line);
        crate::println!("{}", line);
    }
}

pub fn print_netstat() {
    let info = "\
Proto Recv-Q Send-Q Local Address          Foreign Address        State
tcp        0      0 127.0.0.1:80           0.0.0.0:*              LISTEN
tcp        0      0 192.168.1.100:22       192.168.1.1:54321      ESTABLISHED
udp        0      0 0.0.0.0:68             0.0.0.0:*";
    crate::serial_println!("{}", info);
    crate::println!("{}", info);
}
