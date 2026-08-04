#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluFlags {
    pub negative: bool,
    pub zero: bool,
    pub carry: bool,
    pub overflow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddResult {
    pub value: u32,
    pub flags: AluFlags,
}

/// Performs:
///
/// ```text
/// x + y + carry_in
/// ```
///
/// and calculates ARM-compatible N, Z, C and V flags.
///
/// This single primitive can implement both addition and subtraction:
///
/// ADD: x + y
/// ADC: x + y + C
/// SUB: x + NOT(y) + 1
/// SBC: x + NOT(y) + C
/// RSB: y + NOT(x) + 1
/// RSC: y + NOT(x) + C
pub const fn add_with_carry(x: u32, y: u32, carry_in: bool) -> AddResult {
    let carry = carry_in as u64;

    let unsigned_sum = x as u64 + y as u64 + carry;

    let value = unsigned_sum as u32;

    /*
     * Unsigned carry occurs when the mathematical result does not fit
     * inside 32 bits.
     */
    let carry_out = unsigned_sum > u32::MAX as u64;

    /*
     * Signed overflow:
     *
     * x and y have the same sign, but the result has a different sign.
     *
     * The carry input is at most one and does not require a separate
     * sign term. Overflow is correctly determined from the two 32-bit
     * operands and the final 32-bit result.
     */
    let overflow = ((x ^ value) & (y ^ value) & 0x8000_0000) != 0;

    AddResult {
        value,
        flags: AluFlags {
            negative: value & 0x8000_0000 != 0,
            zero: value == 0,
            carry: carry_out,
            overflow,
        },
    }
}

/// Flags produced by logical operations.
///
/// Logical data-processing instructions:
///
/// - derive N and Z from the result
/// - derive C from the barrel shifter carry-out
/// - leave V unchanged
pub const fn logical_flags(value: u32, shifter_carry: bool, old_overflow: bool) -> AluFlags {
    AluFlags {
        negative: value & 0x8000_0000 != 0,
        zero: value == 0,
        carry: shifter_carry,
        overflow: old_overflow,
    }
}

/// Convenience helper for arithmetic operations.
///
/// Arithmetic operations use all four flags returned by
/// `add_with_carry`.
pub const fn arithmetic_shift_flags(result: AddResult) -> AluFlags {
    result.flags
}

#[cfg(test)]
mod tests {
    use super::{AddResult, AluFlags, add_with_carry, logical_flags};

    #[test]
    fn adds_two_values() {
        assert_eq!(
            add_with_carry(1, 2, false),
            AddResult {
                value: 3,
                flags: AluFlags {
                    negative: false,
                    zero: false,
                    carry: false,
                    overflow: false,
                },
            }
        );
    }

    #[test]
    fn addition_can_produce_unsigned_carry() {
        assert_eq!(
            add_with_carry(0xFFFF_FFFF, 1, false),
            AddResult {
                value: 0,
                flags: AluFlags {
                    negative: false,
                    zero: true,
                    carry: true,
                    overflow: false,
                },
            }
        );
    }

    #[test]
    fn addition_can_produce_signed_overflow() {
        /*
         * i32::MAX + 1 = i32::MIN
         */
        assert_eq!(
            add_with_carry(0x7FFF_FFFF, 1, false),
            AddResult {
                value: 0x8000_0000,
                flags: AluFlags {
                    negative: true,
                    zero: false,
                    carry: false,
                    overflow: true,
                },
            }
        );
    }

    #[test]
    fn adding_two_negative_values_can_overflow() {
        /*
         * i32::MIN + (-1) wraps to i32::MAX.
         */
        assert_eq!(
            add_with_carry(0x8000_0000, 0xFFFF_FFFF, false,),
            AddResult {
                value: 0x7FFF_FFFF,
                flags: AluFlags {
                    negative: false,
                    zero: false,
                    carry: true,
                    overflow: true,
                },
            }
        );
    }

    #[test]
    fn carry_in_participates_in_addition() {
        assert_eq!(add_with_carry(1, 2, true).value, 4);
    }

    #[test]
    fn subtraction_without_borrow() {
        /*
         * 5 - 3
         *
         * 5 + NOT(3) + 1
         */
        let result = add_with_carry(5, !3, true);

        assert_eq!(result.value, 2);

        /*
         * C=1 means no unsigned borrow.
         */
        assert!(result.flags.carry);
        assert!(!result.flags.overflow);
    }

    #[test]
    fn subtraction_with_borrow() {
        /*
         * 3 - 5 wraps to 0xFFFF_FFFE.
         */
        let result = add_with_carry(3, !5, true);

        assert_eq!(result.value, 0xFFFF_FFFE);

        /*
         * C=0 means an unsigned borrow occurred.
         */
        assert!(!result.flags.carry);
        assert!(result.flags.negative);
    }

    #[test]
    fn subtraction_can_produce_signed_overflow() {
        /*
         * i32::MIN - 1 wraps to i32::MAX.
         */
        let result = add_with_carry(0x8000_0000, !1, true);

        assert_eq!(result.value, 0x7FFF_FFFF);
        assert!(result.flags.overflow);
    }

    #[test]
    fn subtraction_of_equal_values_is_zero_without_borrow() {
        let result = add_with_carry(0x1234_5678, !0x1234_5678, true);

        assert_eq!(result.value, 0);
        assert!(result.flags.zero);
        assert!(result.flags.carry);
    }

    #[test]
    fn sbc_with_carry_set_has_no_extra_borrow() {
        /*
         * C=1:
         *
         * 5 - 3 - NOT(C)
         * = 5 - 3
         */
        let result = add_with_carry(5, !3, true);

        assert_eq!(result.value, 2);
    }

    #[test]
    fn sbc_with_carry_clear_subtracts_one_more() {
        /*
         * C=0:
         *
         * 5 - 3 - 1
         */
        let result = add_with_carry(5, !3, false);

        assert_eq!(result.value, 1);
    }

    #[test]
    fn logical_flags_use_shifter_carry_and_preserve_overflow() {
        assert_eq!(
            logical_flags(0x8000_0000, true, true,),
            AluFlags {
                negative: true,
                zero: false,
                carry: true,
                overflow: true,
            }
        );
    }

    #[test]
    fn logical_zero_result_sets_zero_flag() {
        assert_eq!(
            logical_flags(0, false, true),
            AluFlags {
                negative: false,
                zero: true,
                carry: false,
                overflow: true,
            }
        );
    }
}
