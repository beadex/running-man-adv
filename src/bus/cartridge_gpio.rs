use chrono::{Datelike, Local, Timelike};

const GPIO_DATA_OFFSET: u32 = 0x0000_00C4;
const GPIO_DIRECTION_OFFSET: u32 = 0x0000_00C6;
const GPIO_CONTROL_OFFSET: u32 = 0x0000_00C8;

const RTC_SCK: u8 = 1 << 0;
const RTC_SIO: u8 = 1 << 1;
const RTC_CS: u8 = 1 << 2;

/* Reset, status, date/time, time, alarm, and three unused commands. */
const RTC_COMMAND_BYTES: [u8; 8] = [0, 1, 7, 3, 2, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl RtcDateTime {
    fn local_now() -> Self {
        let now = Local::now();

        Self {
            year: now.year() as u16,
            month: now.month() as u8,
            day: now.day() as u8,
            weekday: now.weekday().num_days_from_sunday() as u8,
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RtcClock {
    HostLocal,
    Fixed(RtcDateTime),
}

impl RtcClock {
    fn now(self) -> RtcDateTime {
        match self {
            Self::HostLocal => RtcDateTime::local_now(),
            Self::Fixed(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RtcCommand(u8);

impl RtcCommand {
    fn decode(serialized: u8) -> Option<Self> {
        /*
         * S-3511 commands are sent most-significant bit first, while the
         * payload bytes are sent least-significant bit first. The serial
         * accumulator records bits in arrival order, so normalize only the
         * command byte before decoding it.
         */
        let value = serialized.reverse_bits();

        (value >> 4 == 0x06).then_some(Self(value))
    }

    const fn register(self) -> usize {
        ((self.0 >> 1) & 0x07) as usize
    }

    const fn reading(self) -> bool {
        self.0 & 1 != 0
    }
}

#[derive(Debug, Clone)]
struct Rtc {
    clock: RtcClock,
    control: u8,
    transfer_step: u8,
    bits: u8,
    bits_read: u8,
    command: Option<RtcCommand>,
    bytes_remaining: u8,
    time: [u8; 7],
}

impl Rtc {
    fn new() -> Self {
        Self {
            clock: RtcClock::HostLocal,
            control: 0x40,
            transfer_step: 0,
            bits: 0,
            bits_read: 0,
            command: None,
            bytes_remaining: 0,
            time: [0; 7],
        }
    }

    fn reset_protocol(&mut self) {
        self.transfer_step = 0;
        self.bits = 0;
        self.bits_read = 0;
        self.command = None;
        self.bytes_remaining = 0;
    }

    fn observe_pins(&mut self, pin_state: &mut u8, direction: u8) {
        match self.transfer_step {
            0 => {
                if *pin_state & (RTC_SCK | RTC_CS) == RTC_SCK {
                    self.transfer_step = 1;
                }
            }
            1 => {
                if *pin_state & (RTC_SCK | RTC_CS) == RTC_SCK | RTC_CS {
                    self.transfer_step = 2;
                } else if *pin_state & (RTC_SCK | RTC_CS) != RTC_SCK {
                    self.transfer_step = 0;
                }
            }
            2 => {
                if *pin_state & RTC_SCK == 0 {
                    self.bits &= !(1 << self.bits_read);
                    self.bits |= ((*pin_state & RTC_SIO) >> 1) << self.bits_read;
                } else if *pin_state & RTC_CS != 0 {
                    if self.command.is_some_and(RtcCommand::reading) {
                        let output = self.output_bit() << 1;
                        Self::drive_input_pins(pin_state, direction, RTC_SCK | RTC_CS | output);
                        self.bits_read += 1;

                        if self.bits_read == 8 {
                            self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
                            self.bits_read = 0;

                            if self.bytes_remaining == 0 {
                                self.command = None;
                            }
                        }
                    } else {
                        self.bits_read += 1;

                        if self.bits_read == 8 {
                            self.process_byte();
                        }
                    }
                } else {
                    self.reset_protocol();
                    self.transfer_step = u8::from(*pin_state & RTC_SCK != 0);
                    Self::drive_input_pins(pin_state, direction, RTC_SCK);
                }
            }
            _ => unreachable!("RTC transfer step is constrained to 0..=2"),
        }
    }

    fn process_byte(&mut self) {
        if let Some(command) = self.command {
            if command.register() == 1 {
                self.control = self.bits;
            }

            self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
        } else if let Some(command) = RtcCommand::decode(self.bits) {
            let register = command.register();
            self.bytes_remaining = RTC_COMMAND_BYTES[register];

            match register {
                0 => self.control = 0,
                2 | 3 => self.sample_clock(),
                _ => {}
            }

            if self.bytes_remaining != 0 {
                self.command = Some(command);
            }
        }

        self.bits = 0;
        self.bits_read = 0;

        if self.bytes_remaining == 0 {
            self.command = None;
        }
    }

    fn output_bit(&self) -> u8 {
        let Some(command) = self.command else {
            return 0;
        };

        let output_byte = match command.register() {
            1 => self.control,
            2 | 3 => self.time[7 - self.bytes_remaining as usize],
            4 => 0xFF,
            _ => 0,
        };

        (output_byte >> self.bits_read) & 1
    }

    fn sample_clock(&mut self) {
        let now = self.clock.now();
        let hour = if self.control & 0x40 != 0 {
            to_bcd(now.hour)
        } else {
            let hour12 = match now.hour % 12 {
                0 => 12,
                value => value,
            };
            to_bcd(hour12) | if now.hour >= 12 { 0x80 } else { 0 }
        };

        self.time = [
            to_bcd((now.year % 100) as u8),
            to_bcd(now.month),
            to_bcd(now.day),
            to_bcd(now.weekday),
            hour,
            to_bcd(now.minute),
            to_bcd(now.second),
        ];
    }

    fn drive_input_pins(pin_state: &mut u8, direction: u8, value: u8) {
        *pin_state = (*pin_state & direction) | (value & !direction & 0x0F);
    }
}

#[derive(Debug, Clone)]
pub struct GamePakGpio {
    pin_state: u8,
    direction: u8,
    read_enabled: bool,
    rtc: Option<Rtc>,
}

impl GamePakGpio {
    pub fn from_rom(rom: &[u8]) -> Self {
        let rtc = contains_signature(rom, b"SIIRTC_V").then(Rtc::new);

        Self {
            pin_state: 0,
            direction: 0,
            read_enabled: false,
            rtc,
        }
    }

    pub const fn has_device(&self) -> bool {
        self.rtc.is_some()
    }

    pub fn reset_protocol(&mut self) {
        self.pin_state = 0;
        self.direction = 0;
        self.read_enabled = false;

        if let Some(rtc) = &mut self.rtc {
            rtc.reset_protocol();
        }
    }

    pub fn read16(&self, physical_offset: u32) -> Option<u16> {
        if !self.has_device() || !self.read_enabled {
            return None;
        }

        match physical_offset {
            GPIO_DATA_OFFSET => Some(self.pin_state as u16),
            GPIO_DIRECTION_OFFSET => Some(self.direction as u16),
            GPIO_CONTROL_OFFSET => Some(u16::from(self.read_enabled)),
            _ => None,
        }
    }

    pub fn write16(&mut self, physical_offset: u32, value: u16) -> bool {
        if !self.has_device() {
            return false;
        }

        match physical_offset {
            GPIO_DATA_OFFSET => {
                self.pin_state &= !self.direction;
                self.pin_state |= value as u8 & self.direction & 0x0F;

                if let Some(rtc) = &mut self.rtc {
                    rtc.observe_pins(&mut self.pin_state, self.direction);
                }
            }
            GPIO_DIRECTION_OFFSET => self.direction = value as u8 & 0x0F,
            GPIO_CONTROL_OFFSET => self.read_enabled = value & 1 != 0,
            _ => return false,
        }

        true
    }

    #[cfg(test)]
    pub fn set_fixed_rtc_datetime(&mut self, datetime: RtcDateTime) {
        if let Some(rtc) = &mut self.rtc {
            rtc.clock = RtcClock::Fixed(datetime);
        }
    }
}

impl Default for GamePakGpio {
    fn default() -> Self {
        Self::from_rom(&[])
    }
}

const fn to_bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

fn contains_signature(bytes: &[u8], signature: &[u8]) -> bool {
    bytes
        .windows(signature.len())
        .any(|window| window == signature)
}

#[cfg(test)]
mod tests {
    use super::{GamePakGpio, Rtc, RtcClock, RtcCommand, RtcDateTime};

    #[test]
    fn command_decoder_normalizes_msb_first_serial_order() {
        /* Pokémon sends status-read 0x63 MSB-first, accumulated as 0xC6. */
        let command = RtcCommand::decode(0xC6).expect("valid S-3511 command");

        assert_eq!(command.register(), 1);
        assert!(command.reading());
        assert!(RtcCommand::decode(0x63).is_none());
    }

    #[test]
    fn rtc_samples_datetime_as_bcd() {
        let mut rtc = Rtc::new();
        rtc.clock = RtcClock::Fixed(RtcDateTime {
            year: 2024,
            month: 2,
            day: 29,
            weekday: 4,
            hour: 23,
            minute: 59,
            second: 58,
        });
        rtc.sample_clock();

        assert_eq!(rtc.time, [0x24, 0x02, 0x29, 0x04, 0x23, 0x59, 0x58]);
    }

    #[test]
    fn twelve_hour_mode_marks_pm_and_maps_midnight_to_twelve() {
        let mut rtc = Rtc::new();
        rtc.control = 0;
        rtc.clock = RtcClock::Fixed(RtcDateTime {
            year: 2000,
            month: 1,
            day: 1,
            weekday: 6,
            hour: 0,
            minute: 0,
            second: 0,
        });
        rtc.sample_clock();
        assert_eq!(rtc.time[4], 0x12);

        rtc.clock = RtcClock::Fixed(RtcDateTime {
            hour: 13,
            ..rtc.clock.now()
        });
        rtc.sample_clock();
        assert_eq!(rtc.time[4], 0x81);
    }

    #[test]
    fn gpio_only_appears_for_rtc_cartridges() {
        assert!(!GamePakGpio::from_rom(b"FLASH1M_V103").has_device());
        assert!(GamePakGpio::from_rom(b"SIIRTC_V001").has_device());
    }
}
