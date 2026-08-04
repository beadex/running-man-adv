use super::InterruptSource;

pub const TIMER_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerTickResult {
    pub interrupt_requests: u16,
    pub overflows: [u64; TIMER_COUNT],
}

impl TimerTickResult {
    pub const fn new() -> Self {
        Self {
            interrupt_requests: 0,
            overflows: [0; TIMER_COUNT],
        }
    }
}

impl Default for TimerTickResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerIndex {
    Timer0 = 0,
    Timer1 = 1,
    Timer2 = 2,
    Timer3 = 3,
}

impl TimerIndex {
    pub const fn from_usize(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Timer0),
            1 => Some(Self::Timer1),
            2 => Some(Self::Timer2),
            3 => Some(Self::Timer3),
            _ => None,
        }
    }

    pub const fn as_usize(self) -> usize {
        self as usize
    }

    pub const fn interrupt_source(self) -> InterruptSource {
        match self {
            Self::Timer0 => InterruptSource::Timer0,
            Self::Timer1 => InterruptSource::Timer1,
            Self::Timer2 => InterruptSource::Timer2,
            Self::Timer3 => InterruptSource::Timer3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerControl {
    raw: u16,
}

impl TimerControl {
    /*
     * Writable bits:
     *
     * 0..1 = prescaler
     * 2    = cascade
     * 6    = IRQ enable
     * 7    = timer enable
     */
    pub const WRITABLE_MASK: u16 = 0x00C7;

    pub const fn new() -> Self {
        Self { raw: 0 }
    }

    pub const fn from_raw(raw: u16) -> Self {
        Self {
            raw: raw & Self::WRITABLE_MASK,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub const fn prescaler(self) -> u32 {
        match self.raw & 0b11 {
            0b00 => 1,
            0b01 => 64,
            0b10 => 256,
            0b11 => 1024,
            _ => unreachable!(),
        }
    }

    pub const fn cascade(self) -> bool {
        self.raw & (1 << 2) != 0
    }

    pub const fn irq_enabled(self) -> bool {
        self.raw & (1 << 6) != 0
    }

    pub const fn enabled(self) -> bool {
        self.raw & (1 << 7) != 0
    }
}

impl Default for TimerControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    /*
     * Value written to TMxCNT_L.
     *
     * Loaded into `counter` when:
     *
     * - enable transitions 0 -> 1
     * - counter overflows
     */
    reload: u16,

    /*
     * Current readable counter.
     */
    counter: u16,

    control: TimerControl,

    /*
     * CPU cycles accumulated but not yet sufficient to produce a
     * prescaled timer increment.
     */
    prescaler_remainder: u32,
}

impl Timer {
    pub const fn new() -> Self {
        Self {
            reload: 0,
            counter: 0,
            control: TimerControl::new(),
            prescaler_remainder: 0,
        }
    }

    pub const fn reload(self) -> u16 {
        self.reload
    }

    pub const fn counter(self) -> u16 {
        self.counter
    }

    pub const fn control(self) -> TimerControl {
        self.control
    }

    pub fn write_reload(&mut self, value: u16) {
        /*
         * Writing TMxCNT_L changes the reload latch.
         *
         * It does not immediately replace the active counter.
         * The value becomes active at the next enable edge or overflow.
         */
        self.reload = value;
    }

    pub fn write_control(&mut self, value: u16) {
        let old_enabled = self.control.enabled();
        let new_control = TimerControl::from_raw(value);
        let new_enabled = new_control.enabled();

        self.control = new_control;

        if !old_enabled && new_enabled {
            /*
             * Rising enable edge reloads the counter.
             */
            self.counter = self.reload;
            self.prescaler_remainder = 0;
        } else if old_enabled && !new_enabled {
            /*
             * Discard incomplete prescaler progress when disabled.
             */
            self.prescaler_remainder = 0;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advances a directly clocked timer by CPU cycles.
    ///
    /// Returns the number of overflows produced.
    fn tick_direct(&mut self, cycles: u32) -> u64 {
        if !self.control.enabled() {
            return 0;
        }

        let prescaler = self.control.prescaler();

        let accumulated = self.prescaler_remainder as u64 + cycles as u64;

        let increments = accumulated / prescaler as u64;

        self.prescaler_remainder = (accumulated % prescaler as u64) as u32;

        self.advance_counter(increments)
    }

    /// Advances a cascaded timer by a number of previous-timer
    /// overflows.
    fn tick_cascade(&mut self, previous_overflows: u64) -> u64 {
        if !self.control.enabled() || !self.control.cascade() {
            return 0;
        }

        self.advance_counter(previous_overflows)
    }

    /// Advances the 16-bit counter efficiently and returns the number
    /// of overflows.
    fn advance_counter(&mut self, increments: u64) -> u64 {
        if increments == 0 {
            return 0;
        }

        /*
         * Number of increments required to overflow from the current
         * counter value.
         *
         * counter = 0xFFFF requires one increment.
         */
        let increments_to_first_overflow = 0x1_0000u64 - self.counter as u64;

        if increments < increments_to_first_overflow {
            self.counter = self.counter.wrapping_add(increments as u16);

            return 0;
        }

        /*
         * The first overflow reloads the counter.
         */
        let mut overflow_count = 1u64;

        let remaining = increments - increments_to_first_overflow;

        /*
         * After every overflow, the timer starts from reload.
         *
         * Example:
         *
         * reload = 0xFFF0
         * period = 16 increments
         */
        let reload_period = 0x1_0000u64 - self.reload as u64;

        overflow_count += remaining / reload_period;

        let increments_after_last_overflow = remaining % reload_period;

        self.counter = self
            .reload
            .wrapping_add(increments_after_last_overflow as u16);

        overflow_count
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerController {
    timers: [Timer; TIMER_COUNT],
    enabled_mask: u8,
}

impl TimerController {
    pub const fn new() -> Self {
        Self {
            timers: [Timer::new(), Timer::new(), Timer::new(), Timer::new()],
            enabled_mask: 0,
        }
    }

    pub fn timer(&self, index: TimerIndex) -> &Timer {
        &self.timers[index.as_usize()]
    }

    pub fn timer_mut(&mut self, index: TimerIndex) -> &mut Timer {
        &mut self.timers[index.as_usize()]
    }

    pub fn read_counter(&self, index: TimerIndex) -> u16 {
        self.timer(index).counter()
    }

    pub fn read_control(&self, index: TimerIndex) -> u16 {
        self.timer(index).control().raw()
    }

    pub fn write_reload(&mut self, index: TimerIndex, value: u16) {
        self.timer_mut(index).write_reload(value);
    }

    pub fn write_control(&mut self, index: TimerIndex, value: u16) {
        self.timer_mut(index).write_control(value);

        let mask = 1 << index.as_usize();
        if self.timer(index).control().enabled() {
            self.enabled_mask |= mask;
        } else {
            self.enabled_mask &= !mask;
        }
    }

    /// Advances every timer and returns an interrupt-source mask.
    ///
    /// Timer 0 always uses the CPU clock. Timer 1–3 can either use the
    /// CPU clock or increment once for each overflow of the preceding
    /// timer.
    pub fn tick(&mut self, cycles: u32) -> TimerTickResult {
        if self.enabled_mask == 0 {
            return TimerTickResult::new();
        }

        let mut result = TimerTickResult::new();
        let mut previous_overflows = 0u64;
        let last_enabled = 7 - self.enabled_mask.leading_zeros() as usize;

        for index in 0..=last_enabled {
            let timer_index = TimerIndex::from_usize(index).expect("valid timer index");

            let timer = &mut self.timers[index];

            let overflows = if index > 0 && timer.control().cascade() {
                timer.tick_cascade(previous_overflows)
            } else {
                /*
                 * Timer 0 ignores cascade mode and is always directly
                 * clocked.
                 */
                timer.tick_direct(cycles)
            };

            if overflows > 0 && timer.control().irq_enabled() {
                result.interrupt_requests |= timer_index.interrupt_source().mask();
            }

            result.overflows[index] = overflows;
            previous_overflows = overflows;
        }

        result
    }

    pub fn reset(&mut self) {
        for timer in &mut self.timers {
            timer.reset();
        }
        self.enabled_mask = 0;
    }
}

impl Default for TimerController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{TimerControl, TimerController, TimerIndex};

    #[test]
    fn decodes_timer_control() {
        let control = TimerControl::from_raw(0x00C7);

        assert_eq!(control.prescaler(), 1024);
        assert!(control.cascade());
        assert!(control.irq_enabled());
        assert!(control.enabled());
    }

    #[test]
    fn enabling_timer_loads_reload_value() {
        let mut timers = TimerController::new();

        timers.write_reload(TimerIndex::Timer0, 0xFFF0);

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0xFFF0);
    }

    #[test]
    fn disabled_timer_does_not_advance() {
        let mut timers = TimerController::new();

        timers.write_reload(TimerIndex::Timer0, 0x1234);

        timers.tick(1_000);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0);
    }

    #[test]
    fn timer_zero_advances_with_prescaler_one() {
        let mut timers = TimerController::new();

        timers.write_reload(TimerIndex::Timer0, 0);

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        timers.tick(42);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 42);
    }

    #[test]
    fn prescaler_accumulates_partial_cycles() {
        let mut timers = TimerController::new();

        /*
         * Prescaler = 64, enabled.
         */
        timers.write_control(TimerIndex::Timer0, (1 << 7) | 0b01);

        timers.tick(63);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0);

        timers.tick(1);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 1);
    }

    #[test]
    fn overflow_reloads_counter() {
        let mut timers = TimerController::new();

        timers.write_reload(TimerIndex::Timer0, 0xFFFE);

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        timers.tick(2);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0xFFFE);
    }

    #[test]
    fn overflow_requests_timer_interrupt() {
        let mut timers = TimerController::new();

        timers.write_reload(TimerIndex::Timer0, 0xFFFF);

        timers.write_control(TimerIndex::Timer0, (1 << 7) | (1 << 6));

        let requests = timers.tick(1);

        assert_ne!(
            requests.interrupt_requests & TimerIndex::Timer0.interrupt_source().mask(),
            0
        );
        assert_eq!(requests.overflows[TimerIndex::Timer0.as_usize()], 1);
    }

    #[test]
    fn cascade_timer_increments_on_previous_overflow() {
        let mut timers = TimerController::new();

        /*
         * Timer 0 overflows every cycle.
         */
        timers.write_reload(TimerIndex::Timer0, 0xFFFF);

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        /*
         * Timer 1 is cascaded.
         */
        timers.write_reload(TimerIndex::Timer1, 0);

        timers.write_control(TimerIndex::Timer1, (1 << 7) | (1 << 2));

        timers.tick(10);

        assert_eq!(timers.read_counter(TimerIndex::Timer1), 10);
    }

    #[test]
    fn cascade_overflow_can_propagate_through_multiple_timers() {
        let mut timers = TimerController::new();

        for timer in [TimerIndex::Timer0, TimerIndex::Timer1, TimerIndex::Timer2] {
            timers.write_reload(timer, 0xFFFF);
        }

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        timers.write_control(TimerIndex::Timer1, (1 << 7) | (1 << 2));

        timers.write_control(TimerIndex::Timer2, (1 << 7) | (1 << 2));

        timers.tick(1);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0xFFFF);

        assert_eq!(timers.read_counter(TimerIndex::Timer1), 0xFFFF);

        assert_eq!(timers.read_counter(TimerIndex::Timer2), 0xFFFF);
    }

    #[test]
    fn large_tick_handles_multiple_overflows() {
        let mut timers = TimerController::new();

        /*
         * Period = 16 cycles.
         */
        timers.write_reload(TimerIndex::Timer0, 0xFFF0);

        timers.write_control(TimerIndex::Timer0, 1 << 7);

        timers.tick(16 * 100 + 5);

        assert_eq!(timers.read_counter(TimerIndex::Timer0), 0xFFF5);
    }
}
