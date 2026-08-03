use crate::{
    bus::{Bus, BusLoadError, InterruptController, InterruptSource, IoRegisters},
    cpu::{Cpu, CpuState, Registers},
};

#[derive(Debug)]
pub struct Gba {
    cpu: Cpu,
    bus: Bus,

    /*
     * Total machine cycles elapsed since construction or reset.
     *
     * This is useful for debugging, scheduling video/audio, tracing
     * and deterministic tests.
     */
    elapsed_cycles: u64,

    /*
     * Stops the host-side emulation loop without conflating it with
     * the ARM CPU's HALT state.
     */
    stopped: bool,
}

impl Gba {
    /// Creates an empty GBA machine.
    ///
    /// BIOS and ROM must be loaded separately before execution.
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(),
            elapsed_cycles: 0,
            stopped: false,
        }
    }

    /// Creates a machine and immediately loads BIOS and cartridge ROM.
    pub fn with_images(bios: &[u8], rom: &[u8]) -> Result<Self, BusLoadError> {
        let mut gba = Self::new();

        gba.load_bios(bios)?;
        gba.load_rom(rom)?;

        Ok(gba)
    }

    /// Loads the 16 KiB GBA BIOS image.
    pub fn load_bios(&mut self, bios: &[u8]) -> Result<(), BusLoadError> {
        self.bus.load_bios(bios)
    }

    /// Loads the cartridge ROM image.
    pub fn load_rom(&mut self, rom: &[u8]) -> Result<(), BusLoadError> {
        self.bus.load_rom(rom)
    }

    /// Resets mutable machine state while preserving BIOS and ROM.
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.bus.reset();

        self.elapsed_cycles = 0;
        self.stopped = false;
    }

    /// Advances the machine by one CPU scheduling unit.
    ///
    /// At the current stage, this means:
    ///
    /// 1. CPU samples pending IRQ.
    /// 2. CPU executes one instruction or exception entry.
    /// 3. Timers and other bus peripherals advance by the returned
    ///    cycle count.
    ///
    pub fn step(&mut self) -> u32 {
        if self.stopped {
            return 0;
        }

        /*
         * DMA owns the memory bus while active, so CPU does not execute
         * during this scheduling step.
         */
        let cycles = if let Some(dma_result) = self.bus.run_pending_dma() {
            dma_result.cycles
        } else {
            self.cpu.step(&mut self.bus)
        };

        self.bus.tick(cycles);

        self.elapsed_cycles = self.elapsed_cycles.wrapping_add(cycles as u64);

        cycles
    }

    /// Executes a fixed number of machine scheduling steps.
    ///
    /// This counts calls to `step`, not CPU cycles.
    pub fn run_steps(&mut self, step_count: usize) -> u64 {
        let starting_cycles = self.elapsed_cycles;

        for _ in 0..step_count {
            if self.stopped {
                break;
            }

            self.step();
        }

        self.elapsed_cycles.wrapping_sub(starting_cycles)
    }

    /// Runs until at least `cycle_budget` additional cycles have
    /// elapsed.
    ///
    /// The returned value may be slightly greater than the requested
    /// budget because an instruction cannot generally be split.
    pub fn run_cycles(&mut self, cycle_budget: u64) -> u64 {
        let starting_cycles = self.elapsed_cycles;

        while !self.stopped && self.elapsed_cycles.wrapping_sub(starting_cycles) < cycle_budget {
            let cycles = self.step();

            /*
             * Prevent accidental infinite loops if a future scheduler
             * path returns zero without setting `stopped`.
             */
            if cycles == 0 {
                break;
            }
        }

        self.elapsed_cycles.wrapping_sub(starting_cycles)
    }

    /// Stops the host emulation loop.
    ///
    /// This is separate from the emulated CPU HALT state.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn resume(&mut self) {
        self.stopped = false;
    }

    pub const fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub const fn elapsed_cycles(&self) -> u64 {
        self.elapsed_cycles
    }

    pub const fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub const fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn bus_mut(&mut self) -> &mut Bus {
        &mut self.bus
    }

    pub const fn registers(&self) -> &Registers {
        self.cpu.registers()
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        self.cpu.registers_mut()
    }

    pub fn state(&self) -> CpuState {
        self.cpu.state()
    }

    pub const fn io(&self) -> &IoRegisters {
        self.bus.io()
    }

    pub fn io_mut(&mut self) -> &mut IoRegisters {
        self.bus.io_mut()
    }

    pub const fn interrupt_controller(&self) -> &InterruptController {
        self.bus.interrupt_controller()
    }

    pub fn interrupt_controller_mut(&mut self) -> &mut InterruptController {
        self.bus.interrupt_controller_mut()
    }

    pub fn request_interrupt(&mut self, source: InterruptSource) {
        self.bus.request_interrupt(source);
    }
}

impl Default for Gba {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Gba;

    use crate::{
        bus::{Bus, InterruptSource},
        cpu::{CpuMode, Registers},
    };

    #[test]
    fn new_machine_starts_without_elapsed_cycles() {
        let gba = Gba::new();

        assert_eq!(gba.elapsed_cycles(), 0);
        assert!(!gba.is_stopped());
    }

    #[test]
    fn step_executes_cpu_and_advances_bus_timers() {
        let mut gba = Gba::new();

        /*
         * ARM NOP:
         *
         * MOV R0, R0
         */
        gba.bus_mut().write32(0x0200_0000, 0xE1A0_0000);

        gba.registers_mut().set_pc(0x0200_0000);

        /*
         * Timer 0:
         *
         * reload = 0
         * prescaler = 1
         * enabled
         */
        gba.bus_mut().write16(Bus::REG_TM0CNT_L, 0);

        gba.bus_mut().write16(Bus::REG_TM0CNT_H, 1 << 7);

        let cycles = gba.step();

        assert_eq!(gba.elapsed_cycles(), cycles as u64,);

        assert_eq!(gba.bus().read16(Bus::REG_TM0CNT_L,), cycles as u16,);

        assert_eq!(gba.registers().pc(), 0x0200_0004,);
    }

    #[test]
    fn timer_irq_is_sampled_on_following_step() {
        let mut gba = Gba::new();

        /*
         * Two ARM NOP instructions.
         */
        gba.bus_mut().write32(0x0200_0000, 0xE1A0_0000);

        gba.bus_mut().write32(0x0200_0004, 0xE1A0_0000);

        gba.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        gba.registers_mut().cpsr_mut().set_irq_disabled(false);

        gba.registers_mut().set_pc(0x0200_0000);

        gba.bus_mut()
            .write16(Bus::REG_IE, InterruptSource::Timer0.mask());

        gba.bus_mut().write16(Bus::REG_IME, 1);

        /*
         * Overflow after one timer increment.
         */
        gba.bus_mut().write16(Bus::REG_TM0CNT_L, 0xFFFF);

        gba.bus_mut()
            .write16(Bus::REG_TM0CNT_H, (1 << 7) | (1 << 6));

        /*
         * Execute one instruction, then Gba::step ticks Timer0.
         */
        gba.step();

        assert_eq!(gba.registers().mode(), CpuMode::System,);

        assert!(gba.bus().irq_line());

        /*
         * IRQ is sampled before the next instruction fetch.
         */
        gba.step();

        assert_eq!(gba.registers().mode(), CpuMode::Irq,);

        assert_eq!(gba.registers().pc(), 0x0000_0018,);

        assert_eq!(gba.registers().read(Registers::LR), 0x0200_0008,);
    }

    #[test]
    fn run_steps_returns_consumed_cycles() {
        let mut gba = Gba::new();

        for index in 0..4 {
            gba.bus_mut().write32(0x0200_0000 + index * 4, 0xE1A0_0000);
        }

        gba.registers_mut().set_pc(0x0200_0000);

        let cycles = gba.run_steps(4);

        assert_eq!(cycles, gba.elapsed_cycles(),);

        assert_eq!(gba.registers().pc(), 0x0200_0010,);
    }

    #[test]
    fn stopped_machine_does_not_advance() {
        let mut gba = Gba::new();

        gba.stop();

        assert_eq!(gba.step(), 0);
        assert_eq!(gba.elapsed_cycles(), 0);
    }

    #[test]
    fn gba_runs_pending_dma_before_cpu() {
        let mut gba = Gba::new();

        /*
         * CPU instruction that must not run while DMA is pending.
         *
         * MOV R0, #42
         */
        gba.bus_mut().write32(0x0200_0000, 0xE3A0_002A);

        gba.registers_mut().set_pc(0x0200_0000);

        gba.bus_mut().write16(0x0200_0100, 0xCAFE);

        gba.bus_mut().write32(Bus::REG_DMA0SAD, 0x0200_0100);

        gba.bus_mut().write32(Bus::REG_DMA0DAD, 0x0300_0100);

        gba.bus_mut().write16(Bus::REG_DMA0CNT_L, 1);

        gba.bus_mut().write16(Bus::REG_DMA0CNT_H, 1 << 15);

        /*
         * DMA runs. CPU must not execute MOV.
         */
        gba.step();

        assert_eq!(gba.bus().read16(0x0300_0100), 0xCAFE);

        assert_eq!(gba.registers().read(0), 0);

        assert_eq!(gba.registers().pc(), 0x0200_0000);

        /*
         * No DMA remains, so CPU now executes MOV.
         */
        gba.step();

        assert_eq!(gba.registers().read(0), 42);

        assert_eq!(gba.registers().pc(), 0x0200_0004);
    }
}
