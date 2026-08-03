use super::InterruptSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Key {
    A = 1 << 0,
    B = 1 << 1,
    Select = 1 << 2,
    Start = 1 << 3,
    Right = 1 << 4,
    Left = 1 << 5,
    Up = 1 << 6,
    Down = 1 << 7,
    R = 1 << 8,
    L = 1 << 9,
}

impl Key {
    pub const fn mask(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeypadInterruptCondition {
    Any,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyControl {
    raw: u16,
}

impl KeyControl {
    pub const KEY_MASK: u16 = 0x03FF;
    pub const IRQ_ENABLE_MASK: u16 = 1 << 14;
    pub const CONDITION_MASK: u16 = 1 << 15;

    pub const WRITABLE_MASK: u16 = Self::KEY_MASK | Self::IRQ_ENABLE_MASK | Self::CONDITION_MASK;

    pub const fn new() -> Self {
        Self { raw: 0 }
    }

    pub const fn from_raw(value: u16) -> Self {
        Self {
            raw: value & Self::WRITABLE_MASK,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub const fn selected_keys(self) -> u16 {
        self.raw & Self::KEY_MASK
    }

    pub const fn irq_enabled(self) -> bool {
        self.raw & Self::IRQ_ENABLE_MASK != 0
    }

    pub const fn condition(self) -> KeypadInterruptCondition {
        if self.raw & Self::CONDITION_MASK != 0 {
            KeypadInterruptCondition::All
        } else {
            KeypadInterruptCondition::Any
        }
    }
}

impl Default for KeyControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeypadUpdateResult {
    pub interrupt_requests: u16,
    pub condition_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keypad {
    /*
     * Internally use active-high pressed bits:
     *
     * bit = 1 means pressed.
     */
    pressed: u16,

    control: KeyControl,

    /*
     * Used to generate the keypad request on a false -> true edge,
     * rather than continuously requesting it every scheduler tick.
     */
    previous_condition_active: bool,
}

impl Keypad {
    pub const KEY_MASK: u16 = 0x03FF;

    pub const fn new() -> Self {
        Self {
            pressed: 0,
            control: KeyControl::new(),
            previous_condition_active: false,
        }
    }

    /// KEYINPUT register value. Bits are active-low.
    pub const fn key_input(&self) -> u16 {
        (!self.pressed) & Self::KEY_MASK
    }

    pub const fn pressed_mask(&self) -> u16 {
        self.pressed
    }

    pub const fn control(&self) -> KeyControl {
        self.control
    }

    pub const fn read_control(&self) -> u16 {
        self.control.raw()
    }

    pub fn write_control(&mut self, value: u16) -> KeypadUpdateResult {
        self.control = KeyControl::from_raw(value);
        self.evaluate_condition()
    }

    pub fn set_key(&mut self, key: Key, pressed: bool) -> KeypadUpdateResult {
        if pressed {
            self.pressed |= key.mask();
        } else {
            self.pressed &= !key.mask();
        }

        self.pressed &= Self::KEY_MASK;

        self.evaluate_condition()
    }

    pub fn set_pressed_mask(&mut self, pressed: u16) -> KeypadUpdateResult {
        self.pressed = pressed & Self::KEY_MASK;
        self.evaluate_condition()
    }

    pub fn release_all(&mut self) -> KeypadUpdateResult {
        self.set_pressed_mask(0)
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn evaluate_condition(&mut self) -> KeypadUpdateResult {
        let selected = self.control.selected_keys();

        /*
         * An empty selected-key mask must not match.
         */
        let condition_active = if selected == 0 {
            false
        } else {
            match self.control.condition() {
                KeypadInterruptCondition::Any => self.pressed & selected != 0,

                KeypadInterruptCondition::All => self.pressed & selected == selected,
            }
        };

        let rising_edge = condition_active && !self.previous_condition_active;

        self.previous_condition_active = condition_active;

        let interrupt_requests = if rising_edge && self.control.irq_enabled() {
            InterruptSource::Keypad.mask()
        } else {
            0
        };

        KeypadUpdateResult {
            interrupt_requests,
            condition_active,
        }
    }
}

impl Default for Keypad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyControl, Keypad, KeypadInterruptCondition};

    use crate::bus::InterruptSource;

    #[test]
    fn all_keys_start_released() {
        let keypad = Keypad::new();

        assert_eq!(keypad.key_input(), 0x03FF,);

        assert_eq!(keypad.pressed_mask(), 0,);
    }

    #[test]
    fn pressed_key_reads_as_zero() {
        let mut keypad = Keypad::new();

        keypad.set_key(Key::A, true);

        assert_eq!(keypad.key_input() & Key::A.mask(), 0,);

        assert_ne!(keypad.key_input() & Key::B.mask(), 0,);
    }

    #[test]
    fn key_control_decodes_any_condition() {
        let control = KeyControl::from_raw(Key::A.mask() | (1 << 14));

        assert_eq!(control.condition(), KeypadInterruptCondition::Any,);

        assert!(control.irq_enabled());
    }

    #[test]
    fn any_selected_key_can_request_interrupt() {
        let mut keypad = Keypad::new();

        keypad.write_control(Key::A.mask() | Key::B.mask() | (1 << 14));

        let result = keypad.set_key(Key::B, true);

        assert_ne!(
            result.interrupt_requests & InterruptSource::Keypad.mask(),
            0,
        );
    }

    #[test]
    fn all_condition_requires_every_selected_key() {
        let mut keypad = Keypad::new();

        keypad.write_control(Key::A.mask() | Key::B.mask() | (1 << 14) | (1 << 15));

        let first = keypad.set_key(Key::A, true);

        assert_eq!(first.interrupt_requests, 0,);

        let second = keypad.set_key(Key::B, true);

        assert_ne!(
            second.interrupt_requests & InterruptSource::Keypad.mask(),
            0,
        );
    }

    #[test]
    fn held_key_does_not_repeatedly_request_interrupt() {
        let mut keypad = Keypad::new();

        keypad.write_control(Key::A.mask() | (1 << 14));

        let first = keypad.set_key(Key::A, true);

        let second = keypad.set_key(Key::A, true);

        assert_ne!(first.interrupt_requests, 0,);

        assert_eq!(second.interrupt_requests, 0,);
    }

    #[test]
    fn condition_can_trigger_again_after_becoming_false() {
        let mut keypad = Keypad::new();

        keypad.write_control(Key::A.mask() | (1 << 14));

        assert_ne!(keypad.set_key(Key::A, true).interrupt_requests, 0,);

        keypad.set_key(Key::A, false);

        assert_ne!(keypad.set_key(Key::A, true).interrupt_requests, 0,);
    }
}
