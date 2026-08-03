pub mod arm;
mod cpsr;
mod registers;

use crate::bus::Bus;

pub use cpsr::Cpsr;
pub use registers::Registers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuState {
    Arm,
    Thumb,
}

#[derive(Debug)]
pub struct Cpu {
    registers: Registers,
    halted: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            halted: false,
        }
    }

    pub fn reset(&mut self) {
        self.registers = Registers::new();
        self.registers.set_pc(0);
        self.registers.cpsr_mut().set_thumb_state(false);
        self.halted = false;
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.halted {
            return 1;
        }

        match self.state() {
            CpuState::Arm => self.step_arm(bus),
            CpuState::Thumb => self.step_thumb(bus),
        }
    }

    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }

    pub fn state(&self) -> CpuState {
        if self.registers.cpsr().thumb_state() {
            CpuState::Thumb
        } else {
            CpuState::Arm
        }
    }

    fn step_arm(&mut self, bus: &mut Bus) -> u32 {
        let instruction_address = self.registers.pc();
        let raw_instruction = bus.read32(instruction_address);

        let condition = arm::condition(raw_instruction);
        let condition_passed = condition.evaluate(self.registers.cpsr());

        /*
         * Temporary sequential PC model.
         *
         * Later, once the ARM pipeline is implemented, the visible PC and
         * instruction address will need to be modelled separately.
         */
        self.registers.set_pc(instruction_address.wrapping_add(4));

        if !condition_passed {
            println!(
                "ARM PC=0x{instruction_address:08X} \
             instruction=0x{raw_instruction:08X} \
             condition={condition:?} skipped"
            );

            /*
             * Temporary cycle count.
             *
             * A failed condition does not execute the instruction, but
             * timing will eventually depend on pipeline and bus behaviour.
             */
            return 1;
        }

        let instruction = match arm::decode_arm(raw_instruction) {
            Ok(instruction) => instruction,

            Err(error) => {
                println!(
                    "ARM decode error: \
                 PC=0x{instruction_address:08X} \
                 instruction=0x{raw_instruction:08X} \
                 error={error:?}"
                );

                return 1;
            }
        };

        match arm::execute_arm(&mut self.registers, &instruction, instruction_address) {
            Ok(()) => {}

            Err(arm::ArmExecutionError::UnimplementedInstruction) => {
                println!(
                    "ARM instruction not implemented: \
             PC=0x{instruction_address:08X} \
             instruction={instruction:?}"
                );
            }

            Err(error) => {
                println!(
                    "ARM execution error: \
             PC=0x{instruction_address:08X} \
             instruction={instruction:?} \
             error={error:?}"
                );
            }
        }

        1
    }

    fn step_thumb(&mut self, bus: &mut Bus) -> u32 {
        let pc = self.registers.pc();
        let instruction = bus.read16(pc);

        println!("THUMB PC=0x{pc:08X} instruction=0x{instruction:04X}");

        self.registers.set_pc(pc.wrapping_add(2));

        // Temporary cycle count.
        1
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Cpu;
    use super::CpuState;
    use super::Registers;
    use crate::bus::Bus;

    #[test]
    fn arm_step_fetches_instruction_and_advances_pc() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        bus.write32(0, 0xE1A0_0000);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.registers().pc(), 4);
    }

    #[test]
    fn failed_conditional_branch_is_not_taken() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * BEQ +0
         *
         * Z is clear, so branch is not taken.
         */
        bus.write32(0x0200_0000, 0x0A00_0000);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.registers_mut().cpsr_mut().set_zero(false);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0004);
    }

    #[test]
    fn passed_conditional_branch_is_taken() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // BEQ +0
        bus.write32(0x0200_0000, 0x0A00_0000);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.registers_mut().cpsr_mut().set_zero(true);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0008);
    }

    #[test]
    fn failed_conditional_bx_does_not_branch() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * BXEQ R0
         *
         * Z is clear, so the instruction is skipped.
         */
        bus.write32(0x0200_0000, 0x012F_FF10);

        cpu.registers_mut().write(0, 0x0800_0101);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.registers_mut().cpsr_mut().set_zero(false);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0004);

        assert_eq!(cpu.state(), CpuState::Arm);
    }

    #[test]
    fn branch_negative_eight_branches_to_itself() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * B .
         *
         * Target = current PC.
         *
         * Architectural branch base is current + 8, so the encoded
         * displacement must be -8.
         *
         * imm24 = -2 = 0xFFFFFE.
         */
        bus.write32(0x0200_0000, 0xEAFF_FFFE);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0000);
    }

    #[test]
    fn cpu_executes_arm_mov_immediate() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * MOV R0, #42
         */
        bus.write32(0x0200_0000, 0xE3A0_002A);

        cpu.registers_mut().set_pc(0x0200_0000);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.registers().read(0), 42);
        assert_eq!(cpu.registers().pc(), 0x0200_0004);
    }

    #[test]
    fn cpu_executes_arm_branch() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * At 0x02000000:
         *
         * B +0
         *
         * Target:
         * current address + 8 + 0
         * = 0x02000008
         */
        bus.write32(0x0200_0000, 0xEA00_0000);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0008);
    }

    #[test]
    fn cpu_executes_arm_branch_with_link() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * BL +4:
         *
         * imm24 = 1
         * displacement = 4
         *
         * target = current + 8 + 4
         *        = 0x0200000C
         *
         * LR = current + 4
         *    = 0x02000004
         */
        bus.write32(0x0200_0000, 0xEB00_0001);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_000C);

        assert_eq!(cpu.registers().read(Registers::LR), 0x0200_0004);
    }

    #[test]
    fn cpu_executes_bx_and_enters_thumb_state() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * BX R0
         */
        bus.write32(0x0200_0000, 0xE12F_FF10);

        cpu.registers_mut().write(0, 0x0800_0101);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0800_0100);

        assert_eq!(cpu.state(), CpuState::Thumb);

        assert!(cpu.registers().cpsr().thumb_state());
    }

    #[test]
    fn cpu_executes_bx_and_stays_in_arm_state() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // BX R0
        bus.write32(0x0200_0000, 0xE12F_FF10);

        cpu.registers_mut().write(0, 0x0800_0102);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0800_0100);

        assert_eq!(cpu.state(), CpuState::Arm);
    }

    #[test]
    fn cpu_executes_arm_mul() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // MUL R0, R1, R2
        bus.write32(0x0200_0000, 0xE000_0291);

        cpu.registers_mut().write(1, 6);
        cpu.registers_mut().write(2, 7);
        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 42);

        assert_eq!(cpu.registers().pc(), 0x0200_0004);
    }

    #[test]
    fn cpu_executes_arm_mla() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // MLA R0, R1, R2, R3
        bus.write32(0x0200_0000, 0xE020_3291);

        cpu.registers_mut().write(1, 6);
        cpu.registers_mut().write(2, 7);
        cpu.registers_mut().write(3, 10);
        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 52);
    }

    #[test]
    fn failed_conditional_mla_is_not_executed() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * MLAEQ R0, R1, R2, R3
         *
         * Z is clear.
         */
        bus.write32(0x0200_0000, 0x0020_3291);

        cpu.registers_mut().write(0, 99);
        cpu.registers_mut().write(1, 6);
        cpu.registers_mut().write(2, 7);
        cpu.registers_mut().write(3, 10);

        cpu.registers_mut().cpsr_mut().set_zero(false);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 99);
    }
}
