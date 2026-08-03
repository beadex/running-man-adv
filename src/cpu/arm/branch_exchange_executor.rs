use crate::cpu::{CpuState, Registers};

use super::BranchExchangeInstruction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchExchangeExecutionResult {
    pub target: u32,
    pub state: CpuState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchExchangeExecutionError {
    ProgramCounterAsSourceNotSupported,
}

pub fn execute_branch_exchange(
    registers: &mut Registers,
    instruction: BranchExchangeInstruction,
) -> Result<BranchExchangeExecutionResult, BranchExchangeExecutionError> {
    if instruction.register as usize == Registers::PC {
        /*
         * BX PC has pipeline-visible PC behavior. Defer it until the
         * pipeline-aware register-read API exists.
         */
        return Err(BranchExchangeExecutionError::ProgramCounterAsSourceNotSupported);
    }

    let raw_target = registers.read(instruction.register as usize);

    let thumb_state = raw_target & 1 != 0;

    let (target, state) = if thumb_state {
        /*
         * Thumb instructions are halfword aligned.
         */
        (raw_target & !1, CpuState::Thumb)
    } else {
        /*
         * ARM instructions are word aligned.
         */
        (raw_target & !3, CpuState::Arm)
    };

    registers.cpsr_mut().set_thumb_state(thumb_state);

    registers.set_pc(target);

    Ok(BranchExchangeExecutionResult { target, state })
}

#[cfg(test)]
mod tests {
    use super::{
        BranchExchangeExecutionError, BranchExchangeExecutionResult, execute_branch_exchange,
    };

    use crate::cpu::{
        CpuState, Registers,
        arm::{ArmCondition, BranchExchangeInstruction},
    };

    #[test]
    fn bx_enters_thumb_state_when_bit_zero_is_set() {
        let mut registers = Registers::new();

        registers.write(0, 0x0800_0101);

        let instruction = BranchExchangeInstruction {
            condition: ArmCondition::Always,
            register: 0,
        };

        let result = execute_branch_exchange(&mut registers, instruction).unwrap();

        assert_eq!(
            result,
            BranchExchangeExecutionResult {
                target: 0x0800_0100,
                state: CpuState::Thumb,
            }
        );

        assert_eq!(registers.pc(), 0x0800_0100);

        assert!(registers.cpsr().thumb_state());
    }

    #[test]
    fn bx_enters_arm_state_when_bit_zero_is_clear() {
        let mut registers = Registers::new();

        registers.cpsr_mut().set_thumb_state(true);

        registers.write(0, 0x0800_0102);

        let instruction = BranchExchangeInstruction {
            condition: ArmCondition::Always,
            register: 0,
        };

        let result = execute_branch_exchange(&mut registers, instruction).unwrap();

        assert_eq!(
            result,
            BranchExchangeExecutionResult {
                target: 0x0800_0100,
                state: CpuState::Arm,
            }
        );

        assert_eq!(registers.pc(), 0x0800_0100);

        assert!(!registers.cpsr().thumb_state());
    }

    #[test]
    fn arm_target_is_word_aligned() {
        let mut registers = Registers::new();

        registers.write(1, 0x0200_0002);

        execute_branch_exchange(
            &mut registers,
            BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: 1,
            },
        )
        .unwrap();

        assert_eq!(registers.pc(), 0x0200_0000);
    }

    #[test]
    fn thumb_target_is_halfword_aligned() {
        let mut registers = Registers::new();

        registers.write(1, 0x0200_0003);

        execute_branch_exchange(
            &mut registers,
            BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: 1,
            },
        )
        .unwrap();

        assert_eq!(registers.pc(), 0x0200_0002);
    }

    #[test]
    fn rejects_pc_as_source_for_now() {
        let mut registers = Registers::new();

        let result = execute_branch_exchange(
            &mut registers,
            BranchExchangeInstruction {
                condition: ArmCondition::Always,
                register: Registers::PC as u8,
            },
        );

        assert_eq!(
            result,
            Err(BranchExchangeExecutionError::ProgramCounterAsSourceNotSupported)
        );
    }
}
