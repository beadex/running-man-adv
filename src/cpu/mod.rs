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
    state: CpuState,
    halted: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            state: CpuState::Arm,
            halted: false,
        }
    }

    pub fn reset(&mut self) {
        self.registers = Registers::new();
        self.registers.set_pc(0);
        self.state = CpuState::Arm;
        self.halted = false;
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.halted {
            return 1;
        }

        match self.state {
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
        self.state
    }

    fn step_arm(&mut self, bus: &mut Bus) -> u32 {
        let pc = self.registers.pc();
        let instruction = bus.read32(pc);

        let condition = arm::condition(instruction);
        let condition_passed = condition.evaluate(self.registers.cpsr());

        /*
         * Temporary sequential PC model.
         *
         * Later, once the ARM pipeline is implemented, the visible PC and
         * instruction address will need to be modelled separately.
         */
        self.registers.set_pc(pc.wrapping_add(4));

        if !condition_passed {
            println!(
                "ARM PC=0x{pc:08X} \
             instruction=0x{instruction:08X} \
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

        let kind = arm::classify(instruction);

        match kind {
            arm::ArmInstructionKind::DataProcessing => {
                let decoded = arm::decode_data_processing(instruction)
                    .map_err(|error| {
                        panic!(
                            "failed to decode ARM instruction \
                         0x{instruction:08X}: {error:?}"
                        )
                    })
                    .unwrap();

                match arm::execute_data_processing(&mut self.registers, decoded) {
                    Ok(()) => {}

                    Err(error) => {
                        println!(
                            "ARM data-processing execution not supported: \
                     PC=0x{pc:08X} \
                     instruction=0x{instruction:08X} \
                     error={error:?}"
                        );
                    }
                }
            }

            _ => {
                println!(
                    "ARM PC=0x{pc:08X} \
                 instruction=0x{instruction:08X} \
                 condition={condition:?} \
                 kind={kind:?}"
                );
            }
        }

        // Execution will be added later.
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
    fn arm_instruction_is_skipped_when_condition_fails() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * ADDEQ R0, R1, R2
         *
         * EQ requires Z == 1.
         */
        bus.write32(0x0200_0000, 0x0081_0002);

        cpu.registers_mut().set_pc(0x0200_0000);
        cpu.registers_mut().cpsr_mut().set_zero(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.registers().pc(), 0x0200_0004);

        /*
         * Execution is not implemented yet, but once it is, this test
         * should additionally assert that R0 remains unchanged.
         */
    }

    #[test]
    fn arm_instruction_is_classified_when_condition_passes() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // ADDEQ R0, R1, R2
        bus.write32(0x0200_0000, 0x0081_0002);

        cpu.registers_mut().set_pc(0x0200_0000);
        cpu.registers_mut().cpsr_mut().set_zero(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 1);
        assert_eq!(cpu.registers().pc(), 0x0200_0004);
    }
}
