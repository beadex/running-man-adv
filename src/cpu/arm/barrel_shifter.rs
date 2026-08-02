use super::ShiftType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftResult {
    pub value: u32,
    pub carry: bool,
}

/// Applies a data-processing immediate shift.
///
/// The encoded shift amount is five bits wide.
///
/// ARM special cases:
///
/// - LSL #0 means no shift and preserves carry.
/// - LSR #0 means LSR #32.
/// - ASR #0 means ASR #32.
/// - ROR #0 means RRX.
pub const fn shift_immediate(
    value: u32,
    shift_type: ShiftType,
    encoded_amount: u8,
    old_carry: bool,
) -> ShiftResult {
    let amount = encoded_amount & 0x1F;

    match shift_type {
        ShiftType::LogicalLeft => logical_shift_left_immediate(value, amount, old_carry),

        ShiftType::LogicalRight => logical_shift_right_immediate(value, amount),

        ShiftType::ArithmeticRight => arithmetic_shift_right_immediate(value, amount),

        ShiftType::RotateRight => rotate_right_immediate(value, amount, old_carry),
    }
}

/// Applies a register-controlled data-processing shift.
///
/// ARM uses the bottom eight bits of Rs as the shift amount.
///
/// Unlike immediate shifts, an amount of zero always means:
///
/// - result unchanged
/// - carry preserved
pub const fn shift_register(
    value: u32,
    shift_type: ShiftType,
    rs_value: u32,
    old_carry: bool,
) -> ShiftResult {
    let amount = (rs_value & 0xFF) as u8;

    if amount == 0 {
        return ShiftResult {
            value,
            carry: old_carry,
        };
    }

    match shift_type {
        ShiftType::LogicalLeft => logical_shift_left_register(value, amount),

        ShiftType::LogicalRight => logical_shift_right_register(value, amount),

        ShiftType::ArithmeticRight => arithmetic_shift_right_register(value, amount),

        ShiftType::RotateRight => rotate_right_register(value, amount),
    }
}

/// Expands an ARM data-processing rotated immediate.
///
/// The actual rotation is `rotate * 2`.
///
/// Carry behavior:
///
/// - rotate == 0: carry is preserved
/// - rotate != 0: carry is bit 31 of the expanded result
pub const fn expand_rotated_immediate(immediate: u8, rotate: u8, old_carry: bool) -> ShiftResult {
    let rotation = ((rotate & 0x0F) as u32) * 2;

    if rotation == 0 {
        ShiftResult {
            value: immediate as u32,
            carry: old_carry,
        }
    } else {
        let value = (immediate as u32).rotate_right(rotation);

        ShiftResult {
            value,
            carry: value & 0x8000_0000 != 0,
        }
    }
}

const fn logical_shift_left_immediate(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    if amount == 0 {
        return ShiftResult {
            value,
            carry: old_carry,
        };
    }

    let amount = amount as u32;

    ShiftResult {
        value: value << amount,
        carry: bit(value, 32 - amount),
    }
}

const fn logical_shift_right_immediate(value: u32, amount: u8) -> ShiftResult {
    /*
     * Encoded LSR #0 means LSR #32.
     */
    if amount == 0 {
        return ShiftResult {
            value: 0,
            carry: bit(value, 31),
        };
    }

    let amount = amount as u32;

    ShiftResult {
        value: value >> amount,
        carry: bit(value, amount - 1),
    }
}

const fn arithmetic_shift_right_immediate(value: u32, amount: u8) -> ShiftResult {
    /*
     * Encoded ASR #0 means ASR #32.
     */
    if amount == 0 {
        let sign = bit(value, 31);

        return ShiftResult {
            value: if sign { u32::MAX } else { 0 },
            carry: sign,
        };
    }

    let amount = amount as u32;

    ShiftResult {
        value: ((value as i32) >> amount) as u32,
        carry: bit(value, amount - 1),
    }
}

const fn rotate_right_immediate(value: u32, amount: u8, old_carry: bool) -> ShiftResult {
    /*
     * Encoded ROR #0 means RRX.
     */
    if amount == 0 {
        let carry_in = (old_carry as u32) << 31;

        return ShiftResult {
            value: carry_in | (value >> 1),
            carry: bit(value, 0),
        };
    }

    let value = value.rotate_right(amount as u32);

    ShiftResult {
        value,
        carry: bit(value, 31),
    }
}

const fn logical_shift_left_register(value: u32, amount: u8) -> ShiftResult {
    match amount {
        1..=31 => {
            let amount = amount as u32;

            ShiftResult {
                value: value << amount,
                carry: bit(value, 32 - amount),
            }
        }

        32 => ShiftResult {
            value: 0,
            carry: bit(value, 0),
        },

        _ => ShiftResult {
            value: 0,
            carry: false,
        },
    }
}

const fn logical_shift_right_register(value: u32, amount: u8) -> ShiftResult {
    match amount {
        1..=31 => {
            let amount = amount as u32;

            ShiftResult {
                value: value >> amount,
                carry: bit(value, amount - 1),
            }
        }

        32 => ShiftResult {
            value: 0,
            carry: bit(value, 31),
        },

        _ => ShiftResult {
            value: 0,
            carry: false,
        },
    }
}

const fn arithmetic_shift_right_register(value: u32, amount: u8) -> ShiftResult {
    match amount {
        1..=31 => {
            let amount = amount as u32;

            ShiftResult {
                value: ((value as i32) >> amount) as u32,
                carry: bit(value, amount - 1),
            }
        }

        _ => {
            let sign = bit(value, 31);

            ShiftResult {
                value: if sign { u32::MAX } else { 0 },
                carry: sign,
            }
        }
    }
}

const fn rotate_right_register(value: u32, amount: u8) -> ShiftResult {
    /*
     * amount == 0 was already handled by shift_register().
     */
    let rotation = (amount as u32) & 31;

    if rotation == 0 {
        /*
         * Non-zero amount divisible by 32:
         *
         * value remains unchanged
         * carry becomes bit 31
         */
        ShiftResult {
            value,
            carry: bit(value, 31),
        }
    } else {
        let value = value.rotate_right(rotation);

        ShiftResult {
            value,
            carry: bit(value, 31),
        }
    }
}

const fn bit(value: u32, index: u32) -> bool {
    value & (1u32 << index) != 0
}

#[cfg(test)]
mod tests {
    use super::{ShiftResult, expand_rotated_immediate, shift_immediate, shift_register};

    use crate::cpu::arm::ShiftType;

    #[test]
    fn immediate_lsl_zero_preserves_value_and_carry() {
        assert_eq!(
            shift_immediate(0x1234_5678, ShiftType::LogicalLeft, 0, true,),
            ShiftResult {
                value: 0x1234_5678,
                carry: true,
            }
        );
    }

    #[test]
    fn immediate_lsl_one() {
        assert_eq!(
            shift_immediate(0x8000_0001, ShiftType::LogicalLeft, 1, false,),
            ShiftResult {
                value: 0x0000_0002,
                carry: true,
            }
        );
    }

    #[test]
    fn immediate_lsr_zero_means_lsr_32() {
        assert_eq!(
            shift_immediate(0x8000_0001, ShiftType::LogicalRight, 0, false,),
            ShiftResult {
                value: 0,
                carry: true,
            }
        );
    }

    #[test]
    fn immediate_lsr_four() {
        assert_eq!(
            shift_immediate(0x8000_0010, ShiftType::LogicalRight, 4, false,),
            ShiftResult {
                value: 0x0800_0001,
                carry: false,
            }
        );
    }

    #[test]
    fn immediate_asr_zero_sign_extends_negative_value() {
        assert_eq!(
            shift_immediate(0x8000_0000, ShiftType::ArithmeticRight, 0, false,),
            ShiftResult {
                value: 0xFFFF_FFFF,
                carry: true,
            }
        );
    }

    #[test]
    fn immediate_asr_zero_clears_positive_value() {
        assert_eq!(
            shift_immediate(0x7FFF_FFFF, ShiftType::ArithmeticRight, 0, true,),
            ShiftResult {
                value: 0,
                carry: false,
            }
        );
    }

    #[test]
    fn immediate_ror_zero_performs_rrx() {
        assert_eq!(
            shift_immediate(0x0000_0003, ShiftType::RotateRight, 0, true,),
            ShiftResult {
                value: 0x8000_0001,
                carry: true,
            }
        );
    }

    #[test]
    fn immediate_ror_four() {
        assert_eq!(
            shift_immediate(0x0000_0001, ShiftType::RotateRight, 4, false,),
            ShiftResult {
                value: 0x1000_0000,
                carry: false,
            }
        );
    }

    #[test]
    fn register_shift_zero_preserves_value_and_carry() {
        for shift_type in [
            ShiftType::LogicalLeft,
            ShiftType::LogicalRight,
            ShiftType::ArithmeticRight,
            ShiftType::RotateRight,
        ] {
            assert_eq!(
                shift_register(0x8123_4567, shift_type, 0, true,),
                ShiftResult {
                    value: 0x8123_4567,
                    carry: true,
                }
            );
        }
    }

    #[test]
    fn register_lsl_32_uses_bit_zero_as_carry() {
        assert_eq!(
            shift_register(0x0000_0001, ShiftType::LogicalLeft, 32, false,),
            ShiftResult {
                value: 0,
                carry: true,
            }
        );
    }

    #[test]
    fn register_lsl_above_32_clears_value_and_carry() {
        assert_eq!(
            shift_register(0xFFFF_FFFF, ShiftType::LogicalLeft, 33, true,),
            ShiftResult {
                value: 0,
                carry: false,
            }
        );
    }

    #[test]
    fn register_lsr_32_uses_bit_31_as_carry() {
        assert_eq!(
            shift_register(0x8000_0000, ShiftType::LogicalRight, 32, false,),
            ShiftResult {
                value: 0,
                carry: true,
            }
        );
    }

    #[test]
    fn register_lsr_above_32_clears_value_and_carry() {
        assert_eq!(
            shift_register(0xFFFF_FFFF, ShiftType::LogicalRight, 33, true,),
            ShiftResult {
                value: 0,
                carry: false,
            }
        );
    }

    #[test]
    fn register_asr_32_sign_extends() {
        assert_eq!(
            shift_register(0x8000_0000, ShiftType::ArithmeticRight, 32, false,),
            ShiftResult {
                value: 0xFFFF_FFFF,
                carry: true,
            }
        );
    }

    #[test]
    fn register_asr_above_32_sign_extends() {
        assert_eq!(
            shift_register(0x7FFF_FFFF, ShiftType::ArithmeticRight, 100, true,),
            ShiftResult {
                value: 0,
                carry: false,
            }
        );
    }

    #[test]
    fn register_ror_32_preserves_value_and_uses_bit_31_as_carry() {
        assert_eq!(
            shift_register(0x8000_0001, ShiftType::RotateRight, 32, false,),
            ShiftResult {
                value: 0x8000_0001,
                carry: true,
            }
        );
    }

    #[test]
    fn register_shift_uses_lowest_eight_bits_of_rs() {
        /*
         * 0x101 becomes shift amount 1.
         */
        assert_eq!(
            shift_register(0x8000_0000, ShiftType::LogicalLeft, 0x101, false,),
            ShiftResult {
                value: 0,
                carry: true,
            }
        );
    }

    #[test]
    fn rotated_immediate_without_rotation_preserves_carry() {
        assert_eq!(
            expand_rotated_immediate(0x80, 0, true),
            ShiftResult {
                value: 0x0000_0080,
                carry: true,
            }
        );
    }

    #[test]
    fn rotated_immediate_calculates_value_and_carry() {
        /*
         * ROR(0x00000080, 8) = 0x80000000
         */
        assert_eq!(
            expand_rotated_immediate(0x80, 4, false),
            ShiftResult {
                value: 0x8000_0000,
                carry: true,
            }
        );
    }
}
