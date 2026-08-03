use crate::cpu::Registers;

use super::{MultiplyLongInstruction, multiply_iteration_count};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplyLongExecutionResult {
    pub value: u64,

    /// ARM7TDMI multiplier iteration count based on Rs.
    pub multiplier_cycles: u32,
}

pub fn execute_multiply_long(
    registers: &mut Registers,
    instruction: MultiplyLongInstruction,
) -> MultiplyLongExecutionResult {
    /*
     * All source operands must be read before either destination
     * register is written.
     */
    let rm_value = registers.read(instruction.rm as usize);

    let rs_value = registers.read(instruction.rs as usize);

    let accumulator = if instruction.accumulate {
        join_u64(
            registers.read(instruction.rd_hi as usize),
            registers.read(instruction.rd_lo as usize),
        )
    } else {
        0
    };

    let product = if instruction.signed {
        signed_product(rm_value, rs_value)
    } else {
        unsigned_product(rm_value, rs_value)
    };

    /*
     * Accumulation is modulo 2^64.
     *
     * This is correct for both signed and unsigned MLAL forms because
     * both use the same 64-bit two's-complement bit representation.
     */
    let value = if instruction.accumulate {
        product.wrapping_add(accumulator)
    } else {
        product
    };

    let rd_lo_value = value as u32;
    let rd_hi_value = (value >> 32) as u32;

    registers.write(instruction.rd_lo as usize, rd_lo_value);

    registers.write(instruction.rd_hi as usize, rd_hi_value);

    if instruction.set_flags {
        registers.cpsr_mut().set_negative(value & (1u64 << 63) != 0);

        registers.cpsr_mut().set_zero(value == 0);

        /*
         * For long multiply instructions with S set:
         *
         * N = result bit 63
         * Z = result is zero
         * C = architecturally meaningless / destroyed on ARM7TDMI
         * V = preserved
         *
         * We preserve C deterministically for now, matching the policy
         * currently used by MULS/MLAS.
         */
    }

    MultiplyLongExecutionResult {
        value,
        multiplier_cycles: multiply_iteration_count(rs_value),
    }
}

const fn unsigned_product(rm: u32, rs: u32) -> u64 {
    (rm as u64) * (rs as u64)
}

const fn signed_product(rm: u32, rs: u32) -> u64 {
    let lhs = rm as i32 as i64;
    let rhs = rs as i32 as i64;

    lhs.wrapping_mul(rhs) as u64
}

const fn join_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

#[cfg(test)]
mod tests {
    use super::{MultiplyLongExecutionResult, execute_multiply_long};

    use crate::cpu::{
        Registers,
        arm::{ArmCondition, MultiplyLongInstruction},
    };

    fn instruction(signed: bool, accumulate: bool, set_flags: bool) -> MultiplyLongInstruction {
        MultiplyLongInstruction {
            condition: ArmCondition::Always,
            signed,
            accumulate,
            set_flags,
            rd_lo: 0,
            rd_hi: 1,
            rm: 2,
            rs: 3,
        }
    }

    #[test]
    fn executes_umull() {
        let mut registers = Registers::new();

        registers.write(2, 0xFFFF_FFFF);
        registers.write(3, 2);

        let result = execute_multiply_long(&mut registers, instruction(false, false, false));

        assert_eq!(
            result,
            MultiplyLongExecutionResult {
                value: 0x0000_0001_FFFF_FFFE,
                multiplier_cycles: 1,
            }
        );

        assert_eq!(registers.read(0), 0xFFFF_FFFE);

        assert_eq!(registers.read(1), 0x0000_0001);
    }

    #[test]
    fn executes_smull_with_negative_operand() {
        let mut registers = Registers::new();

        /*
         * -1 × 2 = -2
         */
        registers.write(2, 0xFFFF_FFFF);
        registers.write(3, 2);

        let result = execute_multiply_long(&mut registers, instruction(true, false, false));

        assert_eq!(result.value, 0xFFFF_FFFF_FFFF_FFFE);

        assert_eq!(registers.read(0), 0xFFFF_FFFE);

        assert_eq!(registers.read(1), 0xFFFF_FFFF);
    }

    #[test]
    fn signed_and_unsigned_results_differ() {
        let mut unsigned_registers = Registers::new();
        let mut signed_registers = Registers::new();

        unsigned_registers.write(2, 0xFFFF_FFFF);
        unsigned_registers.write(3, 2);

        signed_registers.write(2, 0xFFFF_FFFF);
        signed_registers.write(3, 2);

        let unsigned =
            execute_multiply_long(&mut unsigned_registers, instruction(false, false, false));

        let signed = execute_multiply_long(&mut signed_registers, instruction(true, false, false));

        assert_eq!(unsigned.value, 0x0000_0001_FFFF_FFFE);

        assert_eq!(signed.value, 0xFFFF_FFFF_FFFF_FFFE);
    }

    #[test]
    fn executes_umlal() {
        let mut registers = Registers::new();

        /*
         * Existing accumulator:
         * 0x00000001_00000000
         */
        registers.write(0, 0x0000_0000);
        registers.write(1, 0x0000_0001);

        /*
         * Product:
         * 2 × 3 = 6
         */
        registers.write(2, 2);
        registers.write(3, 3);

        let result = execute_multiply_long(&mut registers, instruction(false, true, false));

        assert_eq!(result.value, 0x0000_0001_0000_0006);

        assert_eq!(registers.read(0), 6);
        assert_eq!(registers.read(1), 1);
    }

    #[test]
    fn executes_smlal_with_negative_product() {
        let mut registers = Registers::new();

        /*
         * Accumulator = 10
         */
        registers.write(0, 10);
        registers.write(1, 0);

        /*
         * -2 × 3 = -6
         *
         * 10 + (-6) = 4
         */
        registers.write(2, (-2i32) as u32);
        registers.write(3, 3);

        let result = execute_multiply_long(&mut registers, instruction(true, true, false));

        assert_eq!(result.value, 4);
        assert_eq!(registers.read(0), 4);
        assert_eq!(registers.read(1), 0);
    }

    #[test]
    fn accumulate_wraps_at_64_bits() {
        let mut registers = Registers::new();

        registers.write(0, 0xFFFF_FFFF);
        registers.write(1, 0xFFFF_FFFF);

        registers.write(2, 1);
        registers.write(3, 1);

        execute_multiply_long(&mut registers, instruction(false, true, false));

        assert_eq!(registers.read(0), 0);
        assert_eq!(registers.read(1), 0);
    }

    #[test]
    fn flag_setting_zero_result_sets_z() {
        let mut registers = Registers::new();

        registers.write(2, 0);
        registers.write(3, 123);

        execute_multiply_long(&mut registers, instruction(false, false, true));

        assert!(registers.cpsr().zero());
        assert!(!registers.cpsr().negative());
    }

    #[test]
    fn flag_setting_negative_result_sets_n_from_bit_63() {
        let mut registers = Registers::new();

        /*
         * Signed -1 × 1 = -1.
         */
        registers.write(2, 0xFFFF_FFFF);
        registers.write(3, 1);

        execute_multiply_long(&mut registers, instruction(true, false, true));

        assert!(!registers.cpsr().zero());
        assert!(registers.cpsr().negative());
    }

    #[test]
    fn flags_use_full_64_bit_result() {
        let mut registers = Registers::new();

        /*
         * Low word is zero, high word is non-zero.
         *
         * The full result must not set Z.
         */
        registers.write(2, 0x0001_0000);
        registers.write(3, 0x0001_0000);

        execute_multiply_long(&mut registers, instruction(false, false, true));

        assert_eq!(registers.read(0), 0);
        assert_eq!(registers.read(1), 1);
        assert!(!registers.cpsr().zero());
    }

    #[test]
    fn overflow_is_preserved() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_overflow(true);

        registers.write(2, 2);
        registers.write(3, 3);

        execute_multiply_long(&mut registers, instruction(false, false, true));

        assert!(registers.cpsr().overflow());
    }

    #[test]
    fn current_policy_preserves_carry() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_carry(true);

        registers.write(2, 2);
        registers.write(3, 3);

        execute_multiply_long(&mut registers, instruction(false, false, true));

        assert!(registers.cpsr().carry());
    }

    #[test]
    fn sources_are_read_before_destinations_are_written() {
        let mut registers = Registers::new();

        /*
         * RdLo overlaps Rm.
         *
         * Even though assemblers may restrict some overlapping forms,
         * the executor must still snapshot all sources first.
         */
        let instruction = MultiplyLongInstruction {
            condition: ArmCondition::Always,
            signed: false,
            accumulate: false,
            set_flags: false,
            rd_lo: 2,
            rd_hi: 1,
            rm: 2,
            rs: 3,
        };

        registers.write(2, 6);
        registers.write(3, 7);

        execute_multiply_long(&mut registers, instruction);

        assert_eq!(registers.read(2), 42);
        assert_eq!(registers.read(1), 0);
    }
}
