pub mod arm;
mod cpsr;
mod exception;
mod exception_handler;
mod mode;
mod registers;

use crate::bus::Bus;

pub use self::cpsr::Cpsr;
pub use self::exception::Exception;
pub use self::exception_handler::{
    ExceptionEntryResult, ExceptionError, ExceptionReturnResult, enter_exception,
    return_from_exception,
};
pub use self::mode::{CpuMode, InvalidCpuMode};
pub use self::registers::{Registers, SpsrAccessError};

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

    pub const fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn enter_halt(&mut self) {
        self.halted = true;
    }

    pub fn wake_from_halt(&mut self) {
        self.halted = false;
    }

    fn try_enter_irq(&mut self, bus: &Bus) -> Option<u32> {
        if !bus.irq_line() {
            return None;
        }

        if self.registers.cpsr().irq_disabled() {
            return None;
        }

        /*
         * At the beginning of step(), PC points at the next
         * instruction that would be executed.
         *
         * IRQ returns using:
         *
         *     SUBS PC, LR, #4
         *
         * Therefore:
         *
         * LR_irq = next instruction address + 4
         */
        let return_address = self.registers.pc().wrapping_add(4);

        enter_exception(&mut self.registers, Exception::Irq, return_address)
            .expect("IRQ mode must provide an SPSR");

        Some(1)
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        /*
         * HALT wake-up is separate from IRQ exception acceptance.
         *
         * An enabled pending source can wake the CPU even when IME is
         * clear or CPSR.I masks IRQ exception entry.
         */
        if self.halted {
            if bus.halt_wake_requested() {
                self.halted = false;
            } else {
                /*
                 * Peripheral clocks continue advancing through Gba::step.
                 */
                return 1;
            }
        }

        /*
         * After waking, attempt actual IRQ exception entry.
         */
        if let Some(cycles) = self.try_enter_irq(bus) {
            return cycles;
        }

        match self.state() {
            CpuState::Arm => self.step_arm(bus),

            CpuState::Thumb => self.step_thumb(bus),
        }
    }

    pub const fn registers(&self) -> &Registers {
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

        match arm::execute_arm(&mut self.registers, bus, &instruction, instruction_address) {
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
    use super::{Cpu, CpuMode, CpuState, Registers};
    use crate::bus::{Bus, InterruptSource};

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

    #[test]
    fn cpu_executes_arm_str_then_ldr() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * 0x02000000: STR R0, [R1]
         * 0x02000004: LDR R2, [R1]
         */
        bus.write32(0x0200_0000, 0xE581_0000);

        bus.write32(0x0200_0004, 0xE591_2000);

        cpu.registers_mut().write(0, 0x1234_5678);

        cpu.registers_mut().write(1, 0x0200_0100);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(bus.read32(0x0200_0100), 0x1234_5678);

        assert_eq!(cpu.registers().read(2), 0x1234_5678);

        assert_eq!(cpu.registers().pc(), 0x0200_0008);
    }

    #[test]
    fn cpu_executes_pc_relative_ldr() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * At 0x02000000:
         *
         * LDR R0, [PC, #0]
         *
         * PC base = instruction address + 8
         *         = 0x02000008
         */
        bus.write32(0x0200_0000, 0xE59F_0000);

        bus.write32(0x0200_0008, 0xCAFE_BABE);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 0xCAFE_BABE);
    }

    #[test]
    fn cpu_executes_post_indexed_ldr() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        // LDR R0, [R1], #4
        bus.write32(0x0200_0000, 0xE491_0004);

        bus.write32(0x0200_0100, 0x1122_3344);

        cpu.registers_mut().write(1, 0x0200_0100);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 0x1122_3344);

        assert_eq!(cpu.registers().read(1), 0x0200_0104);
    }

    #[test]
    fn cpu_executes_strh_then_ldrsh() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * STRH  R1, [R0]
         * LDRSH R2, [R0]
         */
        bus.write32(0x0200_0000, 0xE1C0_10B0);

        bus.write32(0x0200_0004, 0xE1D0_20F0);

        cpu.registers_mut().write(0, 0x0200_0100);

        cpu.registers_mut().write(1, 0x0000_8001);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(bus.read16(0x0200_0100), 0x8001);

        assert_eq!(cpu.registers().read(2), 0xFFFF_8001);

        assert_eq!(cpu.registers().pc(), 0x0200_0008);
    }

    #[test]
    fn cpu_executes_arm_push_and_pop_sequence() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * 0x02000000:
         * STMDB SP!, {R4, R5, LR}
         *
         * 0x02000004:
         * LDMIA SP!, {R4, R5, PC}
         */
        bus.write32(0x0200_0000, 0xE92D_4030);

        bus.write32(0x0200_0004, 0xE8BD_8030);

        cpu.registers_mut().write(Registers::SP, 0x0300_8000);

        cpu.registers_mut().write(4, 0x4444_4444);

        cpu.registers_mut().write(5, 0x5555_5555);

        cpu.registers_mut().write(Registers::LR, 0x0200_0100);

        cpu.registers_mut().set_pc(0x0200_0000);

        /*
         * Push.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(Registers::SP), 0x0300_7FF4);

        /*
         * Destroy the source registers to prove that pop restores them.
         */
        cpu.registers_mut().write(4, 0);
        cpu.registers_mut().write(5, 0);

        /*
         * Pop.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(4), 0x4444_4444);

        assert_eq!(cpu.registers().read(5), 0x5555_5555);

        assert_eq!(cpu.registers().pc(), 0x0200_0100);

        assert_eq!(cpu.registers().read(Registers::SP), 0x0300_8000);
    }

    #[test]
    fn cpu_executes_umull() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * UMULL R0, R1, R2, R3
         */
        bus.write32(0x0200_0000, 0xE081_0392);

        cpu.registers_mut().write(2, 0xFFFF_FFFF);

        cpu.registers_mut().write(3, 2);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 0xFFFF_FFFE);

        assert_eq!(cpu.registers().read(1), 0x0000_0001);

        assert_eq!(cpu.registers().pc(), 0x0200_0004);
    }

    #[test]
    fn cpu_executes_smull() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * SMULL R0, R1, R2, R3
         */
        bus.write32(0x0200_0000, 0xE0C1_0392);

        cpu.registers_mut().write(2, (-2i32) as u32);

        cpu.registers_mut().write(3, 3);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        /*
         * -2 × 3 = -6
         * 64-bit two's complement:
         * FFFFFFFF_FFFFFFFA
         */
        assert_eq!(cpu.registers().read(0), 0xFFFF_FFFA);

        assert_eq!(cpu.registers().read(1), 0xFFFF_FFFF);
    }

    #[test]
    fn cpu_executes_umlal() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * UMLAL R0, R1, R2, R3
         */
        bus.write32(0x0200_0000, 0xE0A1_0392);

        /*
         * Initial 64-bit accumulator:
         * 0x00000001_00000000
         */
        cpu.registers_mut().write(0, 0);
        cpu.registers_mut().write(1, 1);

        cpu.registers_mut().write(2, 2);
        cpu.registers_mut().write(3, 3);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 6);

        assert_eq!(cpu.registers().read(1), 1);
    }

    #[test]
    fn failed_conditional_multiply_long_is_not_executed() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * UMULLEQ R0, R1, R2, R3
         *
         * Z is clear.
         */
        bus.write32(0x0200_0000, 0x0081_0392);

        cpu.registers_mut().write(0, 0xAAAA_AAAA);

        cpu.registers_mut().write(1, 0xBBBB_BBBB);

        cpu.registers_mut().write(2, 6);
        cpu.registers_mut().write(3, 7);

        cpu.registers_mut().cpsr_mut().set_zero(false);

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 0xAAAA_AAAA);

        assert_eq!(cpu.registers().read(1), 0xBBBB_BBBB);
    }

    #[test]
    fn cpu_executes_arm_software_interrupt() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * SWI #0x42
         */
        bus.write32(0x0200_0000, 0xEF00_0042);

        cpu.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        cpu.registers_mut().cpsr_mut().set_irq_disabled(false);

        let old_cpsr = cpu.registers().cpsr();

        cpu.registers_mut().set_pc(0x0200_0000);

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().mode(), CpuMode::Supervisor);

        assert_eq!(cpu.registers().pc(), 0x0000_0008);

        assert_eq!(cpu.registers().read(Registers::LR), 0x0200_0004);

        assert_eq!(cpu.registers().spsr().unwrap(), old_cpsr);

        assert!(cpu.registers().cpsr().irq_disabled());
    }

    #[test]
    fn cpu_enters_swi_handler_and_returns_with_movs_pc_lr() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * Application:
         *
         * 0x02000000: SWI #0
         * 0x02000004: MOV R0, #42
         */
        bus.write32(0x0200_0000, 0xEF00_0000);

        bus.write32(0x0200_0004, 0xE3A0_002A);

        /*
         * Exception vector:
         *
         * 0x00000008: MOVS PC, LR
         */
        let mut bios = vec![0u8; 0x4000];

        /*
         * SWI vector handler:
         *
         * 0x00000008: MOVS PC, LR
         */
        bios[0x08..0x0C].copy_from_slice(&0xE1B0_F00Eu32.to_le_bytes());

        bus.load_bios(&bios).unwrap();

        assert_eq!(bus.read32(0x0000_0008), 0xE1B0_F00E,);

        cpu.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        cpu.registers_mut().set_pc(0x0200_0000);

        /*
         * SWI entry.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0000_0008);

        assert_eq!(cpu.registers().mode(), CpuMode::Supervisor);

        /*
         * MOVS PC, LR.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().pc(), 0x0200_0004);

        assert_eq!(cpu.registers().mode(), CpuMode::System);

        /*
         * Execute instruction following SWI.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 42);
    }

    #[test]
    fn cpu_accepts_irq_from_gba_interrupt_controller() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * Application instruction that must not execute
         * before IRQ entry.
         */
        bus.write32(0x0200_0000, 0xE3A0_002A);

        cpu.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        cpu.registers_mut().cpsr_mut().set_irq_disabled(false);

        cpu.registers_mut().set_pc(0x0200_0000);

        let old_cpsr = cpu.registers().cpsr();

        bus.write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        bus.write16(Bus::REG_IME, 1);

        bus.request_interrupt(InterruptSource::Timer0);

        assert!(bus.irq_line());

        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 0);

        assert_eq!(cpu.registers().mode(), CpuMode::Irq);

        assert_eq!(cpu.registers().pc(), 0x0000_0018);

        assert_eq!(cpu.registers().read(Registers::LR), 0x0200_0004);

        assert_eq!(cpu.registers().spsr().unwrap(), old_cpsr);

        assert!(cpu.registers().cpsr().irq_disabled());

        /*
         * IF is not automatically cleared.
         */
        assert_eq!(bus.read16(Bus::REG_IF), InterruptSource::Timer0.mask());
    }

    #[test]
    fn irq_handler_acknowledges_if_and_returns() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        let mut bios = vec![0u8; 0x4000];

        /*
         * IRQ vector:
         *
         * SUBS PC, LR, #4
         */
        bios[0x18..0x1C].copy_from_slice(&0xE25E_F004u32.to_le_bytes());

        bus.load_bios(&bios).unwrap();

        /*
         * Interrupted instruction:
         *
         * MOV R0, #42
         */
        bus.write32(0x0200_0000, 0xE3A0_002A);

        cpu.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        cpu.registers_mut().cpsr_mut().set_irq_disabled(false);

        cpu.registers_mut().set_pc(0x0200_0000);

        bus.write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        bus.write16(Bus::REG_IME, 1);

        bus.request_interrupt(InterruptSource::Timer0);

        /*
         * IRQ entry.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().mode(), CpuMode::Irq);

        /*
         * Handler acknowledges Timer0 by writing one to IF.
         */
        bus.write16(Bus::REG_IF, InterruptSource::Timer0.mask());

        assert!(!bus.irq_line());

        /*
         * Execute SUBS PC, LR, #4.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().mode(), CpuMode::System);

        assert_eq!(cpu.registers().pc(), 0x0200_0000);

        /*
         * Execute interrupted instruction.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().read(0), 42);
    }

    #[test]
    fn timer_overflow_enters_irq_on_next_cpu_step() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();

        /*
         * NOP at application address.
         */
        bus.write32(0x0200_0000, 0xE1A0_0000);

        cpu.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        cpu.registers_mut().cpsr_mut().set_irq_disabled(false);

        cpu.registers_mut().set_pc(0x0200_0000);

        /*
         * Enable Timer0 interrupt in the GBA interrupt controller.
         */
        bus.write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        bus.write16(Bus::REG_IME, 1);

        /*
         * Overflow after one CPU cycle.
         */
        bus.write16(Bus::REG_TM0CNT_L, 0xFFFF);

        bus.write16(Bus::REG_TM0CNT_H, (1 << 7) | (1 << 6));

        /*
         * Execute one CPU instruction.
         */
        let cycles = cpu.step(&mut bus);

        assert_eq!(cpu.registers().mode(), CpuMode::System);

        /*
         * Advance peripherals by elapsed CPU time.
         */
        bus.tick(cycles);

        assert!(bus.irq_line());

        /*
         * The following CPU step samples IRQ before fetching another
         * application instruction.
         */
        cpu.step(&mut bus);

        assert_eq!(cpu.registers().mode(), CpuMode::Irq);

        assert_eq!(cpu.registers().pc(), 0x0000_0018);

        assert_eq!(
            bus.read16(Bus::REG_IF) & InterruptSource::Timer0.mask(),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn timer_one_can_count_timer_zero_overflows() {
        let mut bus = Bus::new();

        /*
         * Timer0 overflows every CPU cycle.
         */
        bus.write16(Bus::REG_TM0CNT_L, 0xFFFF);

        bus.write16(Bus::REG_TM0CNT_H, 1 << 7);

        /*
         * Timer1 cascade mode.
         */
        bus.write16(Bus::REG_TM1CNT_L, 0);

        bus.write16(Bus::REG_TM1CNT_H, (1 << 7) | (1 << 2));

        bus.tick(100);

        assert_eq!(bus.read16(Bus::REG_TM1CNT_L), 100);
    }
}
