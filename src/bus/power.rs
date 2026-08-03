#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerStateRequest {
    Halt,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerControl {
    post_boot_flag: u8,
    pending_request: Option<PowerStateRequest>,
}

impl PowerControl {
    pub const fn new() -> Self {
        Self {
            post_boot_flag: 0,
            pending_request: None,
        }
    }

    pub const fn post_boot_flag(self) -> u8 {
        self.post_boot_flag
    }

    pub fn write_post_boot_flag(&mut self, value: u8) {
        /*
         * Only bit zero is meaningful.
         */
        self.post_boot_flag = value & 1;
    }

    pub fn write_halt_control(&mut self, value: u8) {
        /*
         * HALTCNT bit 7:
         *
         * 0 -> HALT
         * 1 -> STOP
         */
        self.pending_request = Some(if value & 0x80 != 0 {
            PowerStateRequest::Stop
        } else {
            PowerStateRequest::Halt
        });
    }

    pub const fn pending_request(self) -> Option<PowerStateRequest> {
        self.pending_request
    }

    pub fn take_request(&mut self) -> Option<PowerStateRequest> {
        self.pending_request.take()
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for PowerControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{PowerControl, PowerStateRequest};

    #[test]
    fn halt_control_bit_clear_requests_halt() {
        let mut power = PowerControl::new();

        power.write_halt_control(0);

        assert_eq!(power.take_request(), Some(PowerStateRequest::Halt),);

        assert_eq!(power.take_request(), None,);
    }

    #[test]
    fn halt_control_bit_set_requests_stop() {
        let mut power = PowerControl::new();

        power.write_halt_control(0x80);

        assert_eq!(power.take_request(), Some(PowerStateRequest::Stop),);
    }

    #[test]
    fn post_boot_flag_uses_bit_zero_only() {
        let mut power = PowerControl::new();

        power.write_post_boot_flag(0xFF);

        assert_eq!(power.post_boot_flag(), 1,);
    }
}
