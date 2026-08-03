use crate::cpu::Registers;

use super::MultiplyInstruction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplyExecutionResult {
    pub value: u32,

    /// Internal multiplication iterations used by ARM7TDMI's
    /// early-termination multiplier.
    pub multiplier_cycles: u32,
}

pub fn execute_multiply(
    registers: &mut Registers,
    instruction: MultiplyInstruction,
) -> MultiplyExecutionResult {
    /*
     * Read every source before writing Rd.
     *
     * This keeps the execution order explicit and avoids accidental
     * source corruption if restrictions are relaxed in the future.
     */
    let rm_value = registers.read(instruction.rm as usize);

    let rs_value = registers.read(instruction.rs as usize);

    let accumulator = if instruction.accumulate {
        registers.read(instruction.rn as usize)
    } else {
        0
    };

    /*
     * MUL and MLA retain only the low 32 bits.
     *
     * wrapping_mul and wrapping_add directly express that behavior.
     */
    let product = rm_value.wrapping_mul(rs_value);

    let value = if instruction.accumulate {
        product.wrapping_add(accumulator)
    } else {
        product
    };

    registers.write(instruction.rd as usize, value);

    if instruction.set_flags {
        registers.cpsr_mut().set_negative(value & 0x8000_0000 != 0);

        registers.cpsr_mut().set_zero(value == 0);

        /*
         * On ARMv4 and earlier, C is documented as destroyed for
         * MULS/MLAS, while V is unaffected.
         *
         * For now, preserve C deterministically. This is an explicit
         * approximation until ARM7TDMI multiply-carry behavior is
         * modeled or an undefined-flag policy is introduced.
         *
         * V is deliberately preserved.
         */
    }

    MultiplyExecutionResult {
        value,
        multiplier_cycles: multiply_iteration_count(rs_value),
    }
}

/// Returns ARM7TDMI's `m` value for MUL/MLA timing.
///
/// Early termination depends on the multiplier operand Rs. The upper
/// bits are checked for being all zero or all one.
pub const fn multiply_iteration_count(multiplier: u32) -> u32 {
    let signed = multiplier as i32;

    if signed == (signed as i8) as i32 {
        1
    } else if signed == (signed as i16) as i32 {
        2
    } else if signed == ((signed << 8) >> 8) {
        /*
         * Signed 24-bit range.
         */
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::{MultiplyExecutionResult, execute_multiply, multiply_iteration_count};

    use crate::cpu::{
        Registers,
        arm::{ArmCondition, MultiplyInstruction},
    };

    fn mul(rd: u8, rm: u8, rs: u8, set_flags: bool) -> MultiplyInstruction {
        MultiplyInstruction {
            condition: ArmCondition::Always,
            accumulate: false,
            set_flags,
            rd,
            rn: 0,
            rs,
            rm,
        }
    }

    fn mla(rd: u8, rm: u8, rs: u8, rn: u8, set_flags: bool) -> MultiplyInstruction {
        MultiplyInstruction {
            condition: ArmCondition::Always,
            accumulate: true,
            set_flags,
            rd,
            rn,
            rs,
            rm,
        }
    }

    #[test]
    fn executes_mul() {
        let mut registers = Registers::new();

        registers.write(1, 6);
        registers.write(2, 7);

        let result = execute_multiply(&mut registers, mul(0, 1, 2, false));

        assert_eq!(
            result,
            MultiplyExecutionResult {
                value: 42,
                multiplier_cycles: 1,
            }
        );

        assert_eq!(registers.read(0), 42);
    }

    #[test]
    fn executes_mla() {
        let mut registers = Registers::new();

        registers.write(1, 6);
        registers.write(2, 7);
        registers.write(3, 10);

        execute_multiply(&mut registers, mla(0, 1, 2, 3, false));

        assert_eq!(registers.read(0), 52);
    }

    #[test]
    fn multiplication_keeps_low_32_bits() {
        let mut registers = Registers::new();

        registers.write(1, 0xFFFF_FFFF);
        registers.write(2, 2);

        execute_multiply(&mut registers, mul(0, 1, 2, false));

        assert_eq!(registers.read(0), 0xFFFF_FFFE);
    }

    #[test]
    fn mla_addition_wraps_to_32_bits() {
        let mut registers = Registers::new();

        registers.write(1, 0xFFFF_FFFF);
        registers.write(2, 1);
        registers.write(3, 2);

        execute_multiply(&mut registers, mla(0, 1, 2, 3, false));

        assert_eq!(registers.read(0), 1);
    }

    #[test]
    fn muls_updates_negative_and_zero() {
        let mut registers = Registers::new();

        registers.write(1, 0);
        registers.write(2, 123);

        execute_multiply(&mut registers, mul(0, 1, 2, true));

        assert!(registers.cpsr().zero());
        assert!(!registers.cpsr().negative());

        registers.write(1, 0xFFFF_FFFF);
        registers.write(2, 1);

        execute_multiply(&mut registers, mul(0, 1, 2, true));

        assert!(!registers.cpsr().zero());
        assert!(registers.cpsr().negative());
    }

    #[test]
    fn muls_preserves_overflow() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_overflow(true);

        registers.write(1, 2);
        registers.write(2, 3);

        execute_multiply(&mut registers, mul(0, 1, 2, true));

        assert!(registers.cpsr().overflow());
    }

    #[test]
    fn current_approximation_preserves_carry() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_carry(true);
        registers.write(1, 2);
        registers.write(2, 3);

        execute_multiply(&mut registers, mul(0, 1, 2, true));

        assert!(registers.cpsr().carry());
    }

    #[test]
    fn multiplier_cycle_count_handles_positive_values() {
        assert_eq!(multiply_iteration_count(0x0000_007F), 1);

        assert_eq!(multiply_iteration_count(0x0000_7FFF), 2);

        assert_eq!(multiply_iteration_count(0x007F_FFFF), 3);

        assert_eq!(multiply_iteration_count(0x7FFF_FFFF), 4);
    }

    #[test]
    fn multiplier_cycle_count_handles_negative_values() {
        assert_eq!(multiply_iteration_count(0xFFFF_FF80), 1);

        assert_eq!(multiply_iteration_count(0xFFFF_8000), 2);

        assert_eq!(multiply_iteration_count(0xFF80_0000), 3);

        assert_eq!(multiply_iteration_count(0x8000_0000), 4);
    }
}
