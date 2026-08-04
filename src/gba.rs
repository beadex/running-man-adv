use crate::{
    bus::{
        Bus, BusLoadError, CartridgeSaveLoadError, CartridgeSaveType, InterruptController,
        InterruptSource, IoRegisters, Key, PowerStateRequest,
    },
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

    pub const fn cartridge_save_type(&self) -> CartridgeSaveType {
        self.bus.cartridge_save_type()
    }

    pub const fn cartridge_save_data(&self) -> &[u8] {
        self.bus.cartridge_save_data()
    }

    pub fn load_cartridge_save(&mut self, data: &[u8]) -> Result<(), CartridgeSaveLoadError> {
        self.bus.load_cartridge_save(data)
    }

    pub const fn cartridge_save_dirty(&self) -> bool {
        self.bus.cartridge_save_dirty()
    }

    pub fn mark_cartridge_save_clean(&mut self) {
        self.bus.mark_cartridge_save_clean();
    }

    /// Resets mutable machine state while preserving BIOS and ROM.
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.bus.reset();

        self.elapsed_cycles = 0;
        self.stopped = false;
    }

    pub const SCREEN_WIDTH: usize = crate::bus::SCREEN_WIDTH;

    pub const SCREEN_HEIGHT: usize = crate::bus::SCREEN_HEIGHT;

    pub fn framebuffer(&self) -> &[u32] {
        self.bus.framebuffer()
    }

    pub const fn frame_ready(&self) -> bool {
        self.bus.frame_ready()
    }

    pub fn take_frame_ready(&mut self) -> bool {
        self.bus.take_frame_ready()
    }

    pub const fn frame_number(&self) -> u64 {
        self.bus.frame_number()
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

        let cycles = if let Some(dma_result) = self.bus.run_pending_dma() {
            dma_result.cycles
        } else if self.cpu.is_halted() && !self.bus.halt_wake_requested() {
            /*
             * Advancing HALT one cycle at a time creates hundreds of
             * thousands of host scheduler calls per emulated frame. A small
             * bounded quantum preserves responsive interrupt sampling while
             * removing most of that overhead.
             */
            32
        } else {
            self.cpu.step(&mut self.bus)
        };

        /*
         * Peripheral clocks advance for both CPU and DMA cycles, and also
         * during HALT placeholder cycles.
         */
        self.bus.tick(cycles);

        /*
         * Apply HALTCNT written during the CPU or DMA operation only after
         * that operation has completed.
         */
        self.apply_power_request();

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

    pub fn run_until_frame(&mut self) -> u64 {
        /*
         * A complete GBA frame is 228 scanlines × 1232 cycles = 280,896
         * cycles. Allow two frame periods before giving up so a PPU bug
         * cannot freeze the SDL event loop forever.
         */
        const MAX_CYCLES_WITHOUT_FRAME: u64 = 280_896 * 2;

        let starting_cycles = self.elapsed_cycles;

        /*
         * Discard an old unconsumed frame marker so this method waits for
         * the next frame.
         */
        self.take_frame_ready();

        while !self.stopped {
            let cycles = self.step();

            if self.take_frame_ready() {
                break;
            }

            if cycles == 0 {
                break;
            }

            let consumed_cycles = self.elapsed_cycles.wrapping_sub(starting_cycles);

            if consumed_cycles >= MAX_CYCLES_WITHOUT_FRAME {
                eprintln!("warning: PPU did not produce a frame after {consumed_cycles} cycles");

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

    fn apply_power_request(&mut self) {
        let Some(request) = self.bus.take_power_request() else {
            return;
        };

        match request {
            PowerStateRequest::Halt => {
                self.cpu.enter_halt();
            }

            PowerStateRequest::Stop => {
                /*
                 * STOP requires a more complete low-power clock model.
                 * Treat it as HALT temporarily, but keep the distinction
                 * in the MMIO model.
                 */
                self.cpu.enter_halt();
            }
        }
    }

    pub fn set_key(&mut self, key: Key, pressed: bool) {
        self.bus.set_key(key, pressed);
    }

    pub fn press_key(&mut self, key: Key) {
        self.set_key(key, true);
    }

    pub fn release_key(&mut self, key: Key) {
        self.set_key(key, false);
    }

    pub fn set_pressed_keys(&mut self, pressed_mask: u16) {
        self.bus.set_pressed_keys(pressed_mask);
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
        bus::{Bus, InterruptSource, Key},
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
    fn vblank_irq_runs_guest_handler_and_sets_memory_flag() {
        const APPLICATION: u32 = 0x0200_0000;
        const HANDLER: u32 = 0x0200_0100;
        const FLAG: u32 = 0x0300_0100;

        let mut gba = Gba::new();
        let mut bios = vec![0u8; 0x4000];

        /*
         * Minimal IRQ vector trampoline:
         *
         * 0x18: B   0x20
         * 0x20: LDR R0, [PC]
         * 0x24: BX  R0
         * 0x28: handler address
         */
        bios[0x18..0x1C].copy_from_slice(&0xEA00_0000u32.to_le_bytes());
        bios[0x20..0x24].copy_from_slice(&0xE59F_0000u32.to_le_bytes());
        bios[0x24..0x28].copy_from_slice(&0xE12F_FF10u32.to_le_bytes());
        bios[0x28..0x2C].copy_from_slice(&HANDLER.to_le_bytes());

        gba.load_bios(&bios).unwrap();

        /*
         * Application waits in place while PPU timing reaches VBlank.
         */
        gba.bus_mut().write32(APPLICATION, 0xEAFF_FFFE);

        /*
         * Guest IRQ handler:
         *
         * LDR  R0, [PC, #0x18]  ; FLAG
         * MOV  R1, #1
         * STR  R1, [R0]
         * LDR  R0, [PC, #0x10]  ; IF
         * STRH R1, [R0]         ; acknowledge VBlank
         * SUBS PC, LR, #4       ; exception return
         */
        gba.bus_mut().write32(HANDLER, 0xE59F_0018);
        gba.bus_mut().write32(HANDLER + 0x04, 0xE3A0_1001);
        gba.bus_mut().write32(HANDLER + 0x08, 0xE580_1000);
        gba.bus_mut().write32(HANDLER + 0x0C, 0xE59F_0010);
        gba.bus_mut().write32(HANDLER + 0x10, 0xE1C0_10B0);
        gba.bus_mut().write32(HANDLER + 0x14, 0xE25E_F004);
        gba.bus_mut().write32(HANDLER + 0x20, FLAG);
        gba.bus_mut().write32(HANDLER + 0x24, Bus::REG_IF);

        gba.registers_mut().cpsr_mut().set_mode(CpuMode::System);
        gba.registers_mut().cpsr_mut().set_irq_disabled(false);
        gba.registers_mut().set_pc(APPLICATION);

        gba.bus_mut().write16(Bus::REG_DISPSTAT, 1 << 3);
        gba.bus_mut()
            .write16(Bus::REG_IE, InterruptSource::VBlank.mask());
        gba.bus_mut().write16(Bus::REG_IME, 1);

        gba.run_cycles(250_000);

        assert_eq!(gba.bus().read32(FLAG), 1);
        assert_eq!(
            gba.bus().read16(Bus::REG_IF) & InterruptSource::VBlank.mask(),
            0,
        );
        assert_eq!(gba.registers().mode(), CpuMode::System);
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

    #[test]
    fn guest_can_enter_halt_through_haltcnt() {
        let mut gba = Gba::new();

        /*
         * STRB R0, [R1]
         */
        gba.bus_mut().write32(0x0200_0000, 0xE5C1_0000);

        gba.registers_mut().write(0, 0);

        gba.registers_mut().write(1, Bus::REG_HALTCNT);

        gba.registers_mut().set_pc(0x0200_0000);

        gba.step();

        assert!(gba.cpu().is_halted(),);

        assert_eq!(gba.registers().pc(), 0x0200_0004,);
    }

    #[test]
    fn keypad_interrupt_wakes_halted_cpu() {
        let mut gba = Gba::new();

        /*
         * NOP at resume address.
         */
        gba.bus_mut().write32(0x0200_0000, 0xE1A0_0000);

        gba.registers_mut().set_pc(0x0200_0000);

        /*
         * Enable Keypad in IE.
         *
         * Deliberately leave IME clear to prove that IE&IF can wake HALT
         * without necessarily entering IRQ.
         */
        gba.bus_mut()
            .write16(Bus::REG_IE, InterruptSource::Keypad.mask());

        gba.bus_mut()
            .write16(Bus::REG_KEYCNT, Key::Start.mask() | (1 << 14));

        gba.cpu_mut().enter_halt();

        let old_pc = gba.registers().pc();

        /*
         * No interrupt yet, so CPU remains halted.
         */
        gba.step();

        assert!(gba.cpu().is_halted(),);

        assert_eq!(gba.registers().pc(), old_pc,);

        /*
         * Start press sets IF.Keypad.
         */
        gba.press_key(Key::Start);

        assert!(gba.bus().halt_wake_requested(),);

        /*
         * IME is zero, so CPU wakes but does not enter IRQ.
         * It executes the NOP at the resume address.
         */
        gba.step();

        assert!(!gba.cpu().is_halted(),);

        assert_eq!(gba.registers().pc(), 0x0200_0004,);
    }

    #[test]
    fn keypad_interrupt_can_wake_halt_and_enter_irq() {
        let mut gba = Gba::new();

        gba.registers_mut().cpsr_mut().set_mode(CpuMode::System);

        gba.registers_mut().cpsr_mut().set_irq_disabled(false);

        gba.registers_mut().set_pc(0x0200_0000);

        gba.bus_mut()
            .write16(Bus::REG_IE, InterruptSource::Keypad.mask());

        gba.bus_mut().write16(Bus::REG_IME, 1);

        gba.bus_mut()
            .write16(Bus::REG_KEYCNT, Key::Start.mask() | (1 << 14));

        gba.cpu_mut().enter_halt();

        gba.press_key(Key::Start);

        gba.step();

        assert!(!gba.cpu().is_halted(),);

        assert_eq!(gba.registers().mode(), CpuMode::Irq,);

        assert_eq!(gba.registers().pc(), 0x0000_0018,);

        assert_ne!(
            gba.bus().read16(Bus::REG_IF) & InterruptSource::Keypad.mask(),
            0,
        );
    }

    #[test]
    fn gba_exposes_mode3_framebuffer() {
        let mut gba = Gba::new();

        gba.bus_mut().write16(Bus::REG_DISPCNT, 3);

        gba.bus_mut().write16(0x0600_0000, 0x7C00);

        gba.bus_mut().tick(crate::bus::Ppu::HDRAW_CYCLES as u32);

        assert_eq!(gba.framebuffer()[0], 0xFF00_00FF,);
    }
}
