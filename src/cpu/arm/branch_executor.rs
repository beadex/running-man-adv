use crate::cpu::Registers;

use super::BranchInstruction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchExecutionResult {
    pub target: u32,
    pub link_register: Option<u32>,
}

/// Executes an ARM B or BL instruction.
///
/// `instruction_address` is the address of the branch instruction
/// itself, not the sequential next PC currently stored in the CPU.
pub fn execute_branch(
    registers: &mut Registers,
    instruction: BranchInstruction,
    instruction_address: u32,
) -> BranchExecutionResult {
    /*
     * In ARM state, the architectural PC used by B/BL is:
     *
     * current instruction address + 8
     */
    let branch_base = instruction_address.wrapping_add(8);

    let target = branch_base.wrapping_add_signed(instruction.offset);

    let link_register = if instruction.link {
        /*
         * BL stores the address of the instruction immediately after
         * the branch in LR.
         *
         * Current ARM instruction address + 4.
         */
        let return_address = instruction_address.wrapping_add(4);

        registers.write(Registers::LR, return_address);

        Some(return_address)
    } else {
        None
    };

    /*
     * The project currently stores the address of the next instruction
     * to fetch in R15. A future pipeline implementation will move this
     * behavior into pipeline refill logic.
     */
    registers.set_pc(target);

    BranchExecutionResult {
        target,
        link_register,
    }
}

#[cfg(test)]
mod tests {
    use super::{BranchExecutionResult, execute_branch};

    use crate::cpu::{
        Registers,
        arm::{ArmCondition, BranchInstruction},
    };

    #[test]
    fn branch_uses_instruction_address_plus_eight_as_base() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: 0,
        };

        let result = execute_branch(&mut registers, instruction, 0x0800_0000);

        assert_eq!(
            result,
            BranchExecutionResult {
                target: 0x0800_0008,
                link_register: None,
            }
        );

        assert_eq!(registers.pc(), 0x0800_0008);
    }

    #[test]
    fn branch_applies_positive_offset() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: 0x40,
        };

        execute_branch(&mut registers, instruction, 0x0800_0000);

        assert_eq!(registers.pc(), 0x0800_0048);
    }

    #[test]
    fn branch_applies_negative_offset() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: -8,
        };

        execute_branch(&mut registers, instruction, 0x0800_0010);

        /*
         * Base = 0x08000018
         * Offset = -8
         * Target = 0x08000010
         */
        assert_eq!(registers.pc(), 0x0800_0010);
    }

    #[test]
    fn branch_to_self_uses_offset_negative_eight() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: -8,
        };

        execute_branch(&mut registers, instruction, 0x0200_0000);

        assert_eq!(registers.pc(), 0x0200_0000);
    }

    #[test]
    fn branch_with_link_writes_next_instruction_to_lr() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: true,
            offset: 0x100,
        };

        let result = execute_branch(&mut registers, instruction, 0x0800_0000);

        assert_eq!(registers.read(Registers::LR), 0x0800_0004);

        assert_eq!(result.link_register, Some(0x0800_0004));

        assert_eq!(registers.pc(), 0x0800_0108);
    }

    #[test]
    fn ordinary_branch_does_not_modify_lr() {
        let mut registers = Registers::new();

        registers.write(Registers::LR, 0xDEAD_BEEF);

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: 0,
        };

        execute_branch(&mut registers, instruction, 0x0800_0000);

        assert_eq!(registers.read(Registers::LR), 0xDEAD_BEEF);
    }

    #[test]
    fn branch_target_wraps_at_u32_boundary() {
        let mut registers = Registers::new();

        let instruction = BranchInstruction {
            condition: ArmCondition::Always,
            link: false,
            offset: 4,
        };

        execute_branch(&mut registers, instruction, 0xFFFF_FFFC);

        /*
         * Base wraps:
         *
         * 0xFFFFFFFC + 8 = 0x00000004
         *
         * Then offset +4 = 0x00000008.
         */
        assert_eq!(registers.pc(), 0x0000_0008);
    }
}
