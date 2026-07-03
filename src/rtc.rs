use x86_64::instructions::port::Port;

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

fn get_update_in_progress_flag() -> u8 {
    unsafe {
        let mut addr_port = Port::<u8>::new(CMOS_ADDR);
        let mut data_port = Port::<u8>::new(CMOS_DATA);
        addr_port.write(0x0A);
        data_port.read() & 0x80
    }
}

fn get_rtc_register(reg: u8) -> u8 {
    unsafe {
        let mut addr_port = Port::<u8>::new(CMOS_ADDR);
        let mut data_port = Port::<u8>::new(CMOS_DATA);
        addr_port.write(reg);
        data_port.read()
    }
}

pub struct RtcTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

pub fn read_rtc() -> RtcTime {
    while get_update_in_progress_flag() != 0 {}

    let mut second = get_rtc_register(0x00);
    let mut minute = get_rtc_register(0x02);
    let mut hour = get_rtc_register(0x04);
    let mut day = get_rtc_register(0x07);
    let mut month = get_rtc_register(0x08);
    let mut year = get_rtc_register(0x09) as u16;
    let register_b = get_rtc_register(0x0B);

    // Convert BCD to binary values if necessary
    if (register_b & 0x04) == 0 {
        second = (second & 0x0F) + ((second / 16) * 10);
        minute = (minute & 0x0F) + ((minute / 16) * 10);
        hour = (hour & 0x0F) + (((hour & 0x70) / 16) * 10) | (hour & 0x80);
        day = (day & 0x0F) + ((day / 16) * 10);
        month = (month & 0x0F) + ((month / 16) * 10);
        year = (year & 0x0F) + ((year / 16) * 10);
    }

    // Convert 12 hour clock to 24 hour clock if necessary
    if (register_b & 0x02) == 0 && (hour & 0x80) != 0 {
        hour = ((hour & 0x7F) + 12) % 24;
    }

    // Calculate the full year
    year += 2000;

    RtcTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}
