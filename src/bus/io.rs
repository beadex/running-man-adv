use super::{
    DmaChannelIndex, DmaController, DmaStartTiming, InterruptController, InterruptSource, Key,
    Keypad, KeypadUpdateResult, PowerControl, PowerStateRequest, Ppu, PpuTickResult,
    TimerController, TimerIndex, Video, WaitControl,
};

#[derive(Debug, Clone)]
pub struct IoRegisters {
    raw: Box<[u8; Self::SIZE]>,
    interrupts: InterruptController,
    timers: TimerController,
    dma: DmaController,
    ppu: Ppu,
    video: Video,
    keypad: Keypad,
    power: PowerControl,
    wait_control: WaitControl,
}

impl IoRegisters {
    pub const BASE: u32 = 0x0400_0000;
    pub const SIZE: usize = 0x400;

    pub const TM0CNT_L_OFFSET: u32 = 0x0100;
    pub const TM0CNT_H_OFFSET: u32 = 0x0102;

    pub const TM1CNT_L_OFFSET: u32 = 0x0104;
    pub const TM1CNT_H_OFFSET: u32 = 0x0106;

    pub const TM2CNT_L_OFFSET: u32 = 0x0108;
    pub const TM2CNT_H_OFFSET: u32 = 0x010A;

    pub const TM3CNT_L_OFFSET: u32 = 0x010C;
    pub const TM3CNT_H_OFFSET: u32 = 0x010E;

    pub const DMA0SAD_OFFSET: u32 = 0x00B0;
    pub const DMA0DAD_OFFSET: u32 = 0x00B4;
    pub const DMA0CNT_L_OFFSET: u32 = 0x00B8;
    pub const DMA0CNT_H_OFFSET: u32 = 0x00BA;

    pub const DMA1SAD_OFFSET: u32 = 0x00BC;
    pub const DMA1DAD_OFFSET: u32 = 0x00C0;
    pub const DMA1CNT_L_OFFSET: u32 = 0x00C4;
    pub const DMA1CNT_H_OFFSET: u32 = 0x00C6;

    pub const DMA2SAD_OFFSET: u32 = 0x00C8;
    pub const DMA2DAD_OFFSET: u32 = 0x00CC;
    pub const DMA2CNT_L_OFFSET: u32 = 0x00D0;
    pub const DMA2CNT_H_OFFSET: u32 = 0x00D2;

    pub const DMA3SAD_OFFSET: u32 = 0x00D4;
    pub const DMA3DAD_OFFSET: u32 = 0x00D8;
    pub const DMA3CNT_L_OFFSET: u32 = 0x00DC;
    pub const DMA3CNT_H_OFFSET: u32 = 0x00DE;

    pub const IE_OFFSET: u32 = 0x0200;
    pub const IF_OFFSET: u32 = 0x0202;
    pub const IME_OFFSET: u32 = 0x0208;

    pub const DISPCNT_OFFSET: u32 = 0x0000;
    pub const DISPSTAT_OFFSET: u32 = 0x0004;
    pub const VCOUNT_OFFSET: u32 = 0x0006;

    pub const BG0CNT_OFFSET: u32 = 0x0008;
    pub const BG1CNT_OFFSET: u32 = 0x000A;
    pub const BG2CNT_OFFSET: u32 = 0x000C;
    pub const BG3CNT_OFFSET: u32 = 0x000E;

    pub const BG0HOFS_OFFSET: u32 = 0x0010;
    pub const BG0VOFS_OFFSET: u32 = 0x0012;
    pub const BG1HOFS_OFFSET: u32 = 0x0014;
    pub const BG1VOFS_OFFSET: u32 = 0x0016;
    pub const BG2HOFS_OFFSET: u32 = 0x0018;
    pub const BG2VOFS_OFFSET: u32 = 0x001A;
    pub const BG3HOFS_OFFSET: u32 = 0x001C;
    pub const BG3VOFS_OFFSET: u32 = 0x001E;

    pub const BLDCNT_OFFSET: u32 = 0x0050;
    pub const BLDALPHA_OFFSET: u32 = 0x0052;
    pub const BLDY_OFFSET: u32 = 0x0054;

    pub const BG2PA_OFFSET: u32 = 0x0020;
    pub const BG2PB_OFFSET: u32 = 0x0022;
    pub const BG2PC_OFFSET: u32 = 0x0024;
    pub const BG2PD_OFFSET: u32 = 0x0026;
    pub const BG2X_L_OFFSET: u32 = 0x0028;
    pub const BG2X_H_OFFSET: u32 = 0x002A;
    pub const BG2Y_L_OFFSET: u32 = 0x002C;
    pub const BG2Y_H_OFFSET: u32 = 0x002E;

    pub const BG3PA_OFFSET: u32 = 0x0030;
    pub const BG3PB_OFFSET: u32 = 0x0032;
    pub const BG3PC_OFFSET: u32 = 0x0034;
    pub const BG3PD_OFFSET: u32 = 0x0036;
    pub const BG3X_L_OFFSET: u32 = 0x0038;
    pub const BG3X_H_OFFSET: u32 = 0x003A;
    pub const BG3Y_L_OFFSET: u32 = 0x003C;
    pub const BG3Y_H_OFFSET: u32 = 0x003E;

    pub const KEYINPUT_OFFSET: u32 = 0x0130;
    pub const KEYCNT_OFFSET: u32 = 0x0132;

    pub const POSTFLG_OFFSET: u32 = 0x0300;
    pub const HALTCNT_OFFSET: u32 = 0x0301;

    pub const WAITCNT_OFFSET: u32 = 0x0204;

    pub fn new() -> Self {
        Self {
            raw: Box::new([0; Self::SIZE]),
            interrupts: InterruptController::new(),
            timers: TimerController::new(),
            dma: DmaController::new(),
            ppu: Ppu::new(),
            video: Video::new(),
            keypad: Keypad::new(),
            power: PowerControl::new(),
            wait_control: WaitControl::new(),
        }
    }

    pub const fn contains_address(address: u32) -> bool {
        address >= Self::BASE && address < Self::BASE + Self::SIZE as u32
    }

    pub const fn address_to_offset(address: u32) -> u32 {
        address - Self::BASE
    }

    pub const fn interrupts(&self) -> &InterruptController {
        &self.interrupts
    }

    pub fn interrupts_mut(&mut self) -> &mut InterruptController {
        &mut self.interrupts
    }

    pub const fn timers(&self) -> &TimerController {
        &self.timers
    }

    pub fn timers_mut(&mut self) -> &mut TimerController {
        &mut self.timers
    }

    pub const fn dma(&self) -> &DmaController {
        &self.dma
    }

    pub fn dma_mut(&mut self) -> &mut DmaController {
        &mut self.dma
    }

    pub const fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }

    pub const fn video(&self) -> &Video {
        &self.video
    }

    pub fn video_mut(&mut self) -> &mut Video {
        &mut self.video
    }

    pub const fn keypad(&self) -> &Keypad {
        &self.keypad
    }

    pub fn keypad_mut(&mut self) -> &mut Keypad {
        &mut self.keypad
    }

    pub const fn power(&self) -> &PowerControl {
        &self.power
    }

    pub fn power_mut(&mut self) -> &mut PowerControl {
        &mut self.power
    }

    pub fn take_power_request(&mut self) -> Option<PowerStateRequest> {
        self.power.take_request()
    }

    pub fn set_key(&mut self, key: Key, pressed: bool) {
        let result = self.keypad.set_key(key, pressed);

        self.apply_keypad_result(result);
    }

    pub fn set_pressed_keys(&mut self, pressed_mask: u16) {
        let result = self.keypad.set_pressed_mask(pressed_mask);

        self.apply_keypad_result(result);
    }

    fn apply_keypad_result(&mut self, result: KeypadUpdateResult) {
        if result.interrupt_requests != 0 {
            self.interrupts.request_mask(result.interrupt_requests);
        }
    }

    pub const fn wait_control(&self) -> &WaitControl {
        &self.wait_control
    }

    pub fn wait_control_mut(&mut self) -> &mut WaitControl {
        &mut self.wait_control
    }

    pub fn tick(&mut self, cycles: u32) -> PpuTickResult {
        let ppu_result = self.ppu.tick(cycles);

        if ppu_result.hblank_starts != 0 {
            self.dma
                .trigger(DmaStartTiming::HBlank, ppu_result.hblank_starts);
        }

        if ppu_result.vblank_starts != 0 {
            self.dma
                .trigger(DmaStartTiming::VBlank, ppu_result.vblank_starts);
        }

        if ppu_result.interrupt_requests != 0 {
            self.interrupts.request_mask(ppu_result.interrupt_requests);
        }

        let timer_interrupts = self.timers.tick(cycles);

        if timer_interrupts != 0 {
            self.interrupts.request_mask(timer_interrupts);
        }

        ppu_result
    }

    pub const fn irq_line(&self) -> bool {
        self.interrupts.irq_line()
    }

    pub fn request_interrupt(&mut self, source: InterruptSource) {
        self.interrupts.request(source);
    }

    pub fn reset(&mut self) {
        self.raw.fill(0);

        self.interrupts.reset();
        self.timers.reset();
        self.dma.reset();
        self.ppu.reset();
        self.video.reset();
        self.keypad.reset();
        self.power.reset();
        self.wait_control.reset();
    }

    pub fn read8(&self, offset: u32) -> u8 {
        if offset == Self::POSTFLG_OFFSET {
            return self.power.post_boot_flag();
        }

        if offset == Self::HALTCNT_OFFSET {
            /*
             * HALTCNT is write-only.
             */
            return 0;
        }

        let aligned = offset & !1;

        if matches!(
            aligned,
            Self::DISPCNT_OFFSET
                | Self::DISPSTAT_OFFSET
                | Self::VCOUNT_OFFSET
                | Self::BG0CNT_OFFSET
                | Self::BG1CNT_OFFSET
                | Self::BG2CNT_OFFSET
                | Self::BG3CNT_OFFSET
                | Self::BG0HOFS_OFFSET
                | Self::BG0VOFS_OFFSET
                | Self::BG1HOFS_OFFSET
                | Self::BG1VOFS_OFFSET
                | Self::BG2HOFS_OFFSET
                | Self::BG2VOFS_OFFSET
                | Self::BG3HOFS_OFFSET
                | Self::BG3VOFS_OFFSET
                | Self::BLDCNT_OFFSET
                | Self::BLDALPHA_OFFSET
                | Self::BLDY_OFFSET
                | Self::BG2PA_OFFSET
                | Self::BG2PB_OFFSET
                | Self::BG2PC_OFFSET
                | Self::BG2PD_OFFSET
                | Self::BG2X_L_OFFSET
                | Self::BG2X_H_OFFSET
                | Self::BG2Y_L_OFFSET
                | Self::BG2Y_H_OFFSET
                | Self::BG3PA_OFFSET
                | Self::BG3PB_OFFSET
                | Self::BG3PC_OFFSET
                | Self::BG3PD_OFFSET
                | Self::BG3X_L_OFFSET
                | Self::BG3X_H_OFFSET
                | Self::BG3Y_L_OFFSET
                | Self::BG3Y_H_OFFSET
                | Self::KEYINPUT_OFFSET
                | Self::KEYCNT_OFFSET
                | Self::IE_OFFSET
                | Self::IF_OFFSET
                | Self::WAITCNT_OFFSET
                | Self::IME_OFFSET
        ) || decode_dma_register(aligned).is_some()
            || decode_timer_register(aligned).is_some()
        {
            let value = self.read16(aligned);

            return if offset & 1 == 0 {
                value as u8
            } else {
                (value >> 8) as u8
            };
        }

        self.read_raw8(offset)
    }

    pub fn write8(&mut self, offset: u32, value: u8) {
        if offset == Self::POSTFLG_OFFSET {
            self.power.write_post_boot_flag(value);

            return;
        }

        if offset == Self::HALTCNT_OFFSET {
            self.power.write_halt_control(value);

            return;
        }

        let aligned = offset & !1;
        let high_byte = offset & 1 != 0;

        if aligned == Self::DISPCNT_OFFSET {
            if high_byte {
                self.video.write_display_control_high(value);
            } else {
                self.video.write_display_control_low(value);
            }

            return;
        }

        if aligned == Self::DISPSTAT_OFFSET {
            let current = self.ppu.read_dispstat();

            let updated = replace_byte(current, high_byte, value);

            self.ppu.write_dispstat(updated);
            return;
        }

        if aligned == Self::VCOUNT_OFFSET || aligned == Self::KEYINPUT_OFFSET {
            /*
             * Read-only.
             */
            return;
        }

        if is_affine_video_register(aligned) {
            let current = self.read16(aligned);
            let updated = replace_byte(current, high_byte, value);
            self.write16(aligned, updated);
            return;
        }

        if aligned == Self::KEYCNT_OFFSET {
            let current = self.keypad.read_control();

            let updated = replace_byte(current, high_byte, value);

            let result = self.keypad.write_control(updated);

            self.apply_keypad_result(result);
            return;
        }

        if aligned == Self::WAITCNT_OFFSET {
            let current = self.wait_control.raw();

            let updated = replace_byte(current, high_byte, value);

            self.wait_control.write(updated);
            return;
        }

        if let Some((channel, register)) = decode_dma_register(aligned) {
            let current = match register {
                DmaRegister::SourceLow => self.dma.read_source(channel) as u16,

                DmaRegister::SourceHigh => (self.dma.read_source(channel) >> 16) as u16,

                DmaRegister::DestinationLow => self.dma.read_destination(channel) as u16,

                DmaRegister::DestinationHigh => (self.dma.read_destination(channel) >> 16) as u16,

                DmaRegister::Count => self.dma.read_count(channel),

                DmaRegister::Control => self.dma.read_control(channel),
            };

            let updated = replace_byte(current, high_byte, value);

            self.write16(aligned, updated);

            return;
        }

        if let Some((timer, register)) = decode_timer_register(aligned) {
            let current = match register {
                TimerRegister::Counter => {
                    /*
                     * Counter reads expose the active counter, but writes
                     * modify the reload latch.
                     *
                     * Therefore byte merging must use reload, not the
                     * current counter.
                     */
                    self.timers.timer(timer).reload()
                }

                TimerRegister::Control => self.timers.read_control(timer),
            };

            let updated = replace_byte(current, high_byte, value);

            match register {
                TimerRegister::Counter => {
                    self.timers.write_reload(timer, updated);
                }

                TimerRegister::Control => {
                    self.timers.write_control(timer, updated);
                }
            }

            return;
        }

        match aligned {
            Self::IE_OFFSET => {
                let current = self.interrupts.interrupt_enable();

                let updated = replace_byte(current, high_byte, value);

                self.interrupts.set_interrupt_enable(updated);
            }

            Self::IF_OFFSET => {
                /*
                 * Byte-level write-one-to-clear.
                 */
                let mask = if high_byte {
                    (value as u16) << 8
                } else {
                    value as u16
                };

                self.interrupts.acknowledge(mask);
            }

            Self::IME_OFFSET => {
                /*
                 * Only bit zero of the low byte is meaningful.
                 */
                if !high_byte {
                    self.interrupts.set_master_enable(value as u16);
                }
            }

            _ => {
                self.write_raw8(offset, value);
            }
        }
    }

    pub fn read16(&self, offset: u32) -> u16 {
        let offset = offset & !1;

        match offset {
            Self::DISPCNT_OFFSET => {
                return self.video.read_display_control();
            }

            Self::DISPSTAT_OFFSET => {
                return self.ppu.read_dispstat();
            }

            Self::VCOUNT_OFFSET => {
                return self.ppu.vcount();
            }

            Self::BG0CNT_OFFSET => return self.video.read_background_control(0),
            Self::BG1CNT_OFFSET => return self.video.read_background_control(1),
            Self::BG2CNT_OFFSET => return self.video.read_background_control(2),
            Self::BG3CNT_OFFSET => return self.video.read_background_control(3),

            Self::BG0HOFS_OFFSET => return self.video.read_background_horizontal_offset(0),
            Self::BG0VOFS_OFFSET => return self.video.read_background_vertical_offset(0),
            Self::BG1HOFS_OFFSET => return self.video.read_background_horizontal_offset(1),
            Self::BG1VOFS_OFFSET => return self.video.read_background_vertical_offset(1),
            Self::BG2HOFS_OFFSET => return self.video.read_background_horizontal_offset(2),
            Self::BG2VOFS_OFFSET => return self.video.read_background_vertical_offset(2),
            Self::BG3HOFS_OFFSET => return self.video.read_background_horizontal_offset(3),
            Self::BG3VOFS_OFFSET => return self.video.read_background_vertical_offset(3),

            Self::BLDCNT_OFFSET => return self.video.read_blend_control(),
            Self::BLDALPHA_OFFSET => return self.video.read_blend_alpha(),
            Self::BLDY_OFFSET => return self.video.read_blend_brightness(),

            Self::BG2PA_OFFSET => return self.video.affine_background(2).read_pa(),
            Self::BG2PB_OFFSET => return self.video.affine_background(2).read_pb(),
            Self::BG2PC_OFFSET => return self.video.affine_background(2).read_pc(),
            Self::BG2PD_OFFSET => return self.video.affine_background(2).read_pd(),
            Self::BG2X_L_OFFSET => {
                return self.video.affine_background(2).reference_x_raw() as u16;
            }
            Self::BG2X_H_OFFSET => {
                return (self.video.affine_background(2).reference_x_raw() >> 16) as u16;
            }
            Self::BG2Y_L_OFFSET => {
                return self.video.affine_background(2).reference_y_raw() as u16;
            }
            Self::BG2Y_H_OFFSET => {
                return (self.video.affine_background(2).reference_y_raw() >> 16) as u16;
            }

            Self::BG3PA_OFFSET => return self.video.affine_background(3).read_pa(),
            Self::BG3PB_OFFSET => return self.video.affine_background(3).read_pb(),
            Self::BG3PC_OFFSET => return self.video.affine_background(3).read_pc(),
            Self::BG3PD_OFFSET => return self.video.affine_background(3).read_pd(),
            Self::BG3X_L_OFFSET => {
                return self.video.affine_background(3).reference_x_raw() as u16;
            }
            Self::BG3X_H_OFFSET => {
                return (self.video.affine_background(3).reference_x_raw() >> 16) as u16;
            }
            Self::BG3Y_L_OFFSET => {
                return self.video.affine_background(3).reference_y_raw() as u16;
            }
            Self::BG3Y_H_OFFSET => {
                return (self.video.affine_background(3).reference_y_raw() >> 16) as u16;
            }

            Self::KEYINPUT_OFFSET => {
                return self.keypad.key_input();
            }

            Self::KEYCNT_OFFSET => {
                return self.keypad.read_control();
            }

            Self::WAITCNT_OFFSET => {
                return self.wait_control.raw();
            }

            /*
             * POSTFLG and HALTCNT share one halfword.
             *
             * HALTCNT is write-only, so its read value is represented as
             * zero in the high byte.
             */
            Self::POSTFLG_OFFSET => {
                return self.power.post_boot_flag() as u16;
            }

            _ => {}
        }

        if let Some((channel, register)) = decode_dma_register(offset) {
            return match register {
                DmaRegister::SourceLow => self.dma.read_source(channel) as u16,

                DmaRegister::SourceHigh => (self.dma.read_source(channel) >> 16) as u16,

                DmaRegister::DestinationLow => self.dma.read_destination(channel) as u16,

                DmaRegister::DestinationHigh => (self.dma.read_destination(channel) >> 16) as u16,

                /*
                 * DMA count registers are write-only on hardware.
                 * Returning zero is deterministic and suitable here.
                 */
                DmaRegister::Count => 0,

                DmaRegister::Control => self.dma.read_control(channel),
            };
        }

        if let Some((timer, register)) = decode_timer_register(offset) {
            return match register {
                TimerRegister::Counter => self.timers.read_counter(timer),

                TimerRegister::Control => self.timers.read_control(timer),
            };
        }

        match offset {
            Self::IE_OFFSET => self.interrupts.interrupt_enable(),

            Self::IF_OFFSET => self.interrupts.interrupt_flags(),

            Self::IME_OFFSET => self.interrupts.master_enable() as u16,

            _ => {
                let low = self.read_raw8(offset);
                let high = self.read_raw8(offset.wrapping_add(1));

                u16::from_le_bytes([low, high])
            }
        }
    }

    pub fn write16(&mut self, offset: u32, value: u16) {
        let offset = offset & !1;

        match offset {
            Self::DISPCNT_OFFSET => {
                self.video.write_display_control(value);

                return;
            }

            Self::DISPSTAT_OFFSET => {
                self.ppu.write_dispstat(value);
                return;
            }

            Self::BG0CNT_OFFSET => {
                self.video.write_background_control(0, value);
                return;
            }
            Self::BG1CNT_OFFSET => {
                self.video.write_background_control(1, value);
                return;
            }
            Self::BG2CNT_OFFSET => {
                self.video.write_background_control(2, value);
                return;
            }
            Self::BG3CNT_OFFSET => {
                self.video.write_background_control(3, value);
                return;
            }

            Self::BG0HOFS_OFFSET => {
                self.video.write_background_horizontal_offset(0, value);
                return;
            }
            Self::BG0VOFS_OFFSET => {
                self.video.write_background_vertical_offset(0, value);
                return;
            }
            Self::BG1HOFS_OFFSET => {
                self.video.write_background_horizontal_offset(1, value);
                return;
            }
            Self::BG1VOFS_OFFSET => {
                self.video.write_background_vertical_offset(1, value);
                return;
            }
            Self::BG2HOFS_OFFSET => {
                self.video.write_background_horizontal_offset(2, value);
                return;
            }
            Self::BG2VOFS_OFFSET => {
                self.video.write_background_vertical_offset(2, value);
                return;
            }
            Self::BG3HOFS_OFFSET => {
                self.video.write_background_horizontal_offset(3, value);
                return;
            }
            Self::BG3VOFS_OFFSET => {
                self.video.write_background_vertical_offset(3, value);
                return;
            }

            Self::BLDCNT_OFFSET => {
                self.video.write_blend_control(value);
                return;
            }
            Self::BLDALPHA_OFFSET => {
                self.video.write_blend_alpha(value);
                return;
            }
            Self::BLDY_OFFSET => {
                self.video.write_blend_brightness(value);
                return;
            }

            Self::BG2PA_OFFSET => {
                self.video.affine_background_mut(2).write_pa(value);
                return;
            }
            Self::BG2PB_OFFSET => {
                self.video.affine_background_mut(2).write_pb(value);
                return;
            }
            Self::BG2PC_OFFSET => {
                self.video.affine_background_mut(2).write_pc(value);
                return;
            }
            Self::BG2PD_OFFSET => {
                self.video.affine_background_mut(2).write_pd(value);
                return;
            }
            Self::BG2X_L_OFFSET => {
                self.video
                    .affine_background_mut(2)
                    .write_reference_x_low(value);
                return;
            }
            Self::BG2X_H_OFFSET => {
                self.video
                    .affine_background_mut(2)
                    .write_reference_x_high(value);
                return;
            }
            Self::BG2Y_L_OFFSET => {
                self.video
                    .affine_background_mut(2)
                    .write_reference_y_low(value);
                return;
            }
            Self::BG2Y_H_OFFSET => {
                self.video
                    .affine_background_mut(2)
                    .write_reference_y_high(value);
                return;
            }

            Self::BG3PA_OFFSET => {
                self.video.affine_background_mut(3).write_pa(value);
                return;
            }
            Self::BG3PB_OFFSET => {
                self.video.affine_background_mut(3).write_pb(value);
                return;
            }
            Self::BG3PC_OFFSET => {
                self.video.affine_background_mut(3).write_pc(value);
                return;
            }
            Self::BG3PD_OFFSET => {
                self.video.affine_background_mut(3).write_pd(value);
                return;
            }
            Self::BG3X_L_OFFSET => {
                self.video
                    .affine_background_mut(3)
                    .write_reference_x_low(value);
                return;
            }
            Self::BG3X_H_OFFSET => {
                self.video
                    .affine_background_mut(3)
                    .write_reference_x_high(value);
                return;
            }
            Self::BG3Y_L_OFFSET => {
                self.video
                    .affine_background_mut(3)
                    .write_reference_y_low(value);
                return;
            }
            Self::BG3Y_H_OFFSET => {
                self.video
                    .affine_background_mut(3)
                    .write_reference_y_high(value);
                return;
            }

            Self::VCOUNT_OFFSET => {
                /*
                 * Read-only.
                 */
                return;
            }

            Self::KEYINPUT_OFFSET => {
                /*
                 * Read-only.
                 */
                return;
            }

            Self::KEYCNT_OFFSET => {
                let result = self.keypad.write_control(value);

                self.apply_keypad_result(result);
                return;
            }

            Self::WAITCNT_OFFSET => {
                self.wait_control.write(value);
                return;
            }

            Self::POSTFLG_OFFSET => {
                /*
                 * A halfword write covers:
                 *
                 * low byte  -> POSTFLG
                 * high byte -> HALTCNT
                 */
                let [postflg, haltcnt] = value.to_le_bytes();

                self.power.write_post_boot_flag(postflg);

                self.power.write_halt_control(haltcnt);

                return;
            }

            _ => {}
        }

        if let Some((channel, register)) = decode_dma_register(offset) {
            match register {
                DmaRegister::SourceLow => {
                    let current = self.dma.read_source(channel);

                    let updated = (current & 0xFFFF_0000) | value as u32;

                    self.dma.write_source(channel, updated);
                }

                DmaRegister::SourceHigh => {
                    let current = self.dma.read_source(channel);

                    let updated = (current & 0x0000_FFFF) | ((value as u32) << 16);

                    self.dma.write_source(channel, updated);
                }

                DmaRegister::DestinationLow => {
                    let current = self.dma.read_destination(channel);

                    let updated = (current & 0xFFFF_0000) | value as u32;

                    self.dma.write_destination(channel, updated);
                }

                DmaRegister::DestinationHigh => {
                    let current = self.dma.read_destination(channel);

                    let updated = (current & 0x0000_FFFF) | ((value as u32) << 16);

                    self.dma.write_destination(channel, updated);
                }

                DmaRegister::Count => {
                    self.dma.write_count(channel, value);
                }

                DmaRegister::Control => {
                    self.dma.write_control(channel, value);
                }
            }

            return;
        }

        if let Some((timer, register)) = decode_timer_register(offset) {
            match register {
                TimerRegister::Counter => {
                    self.timers.write_reload(timer, value);
                }

                TimerRegister::Control => {
                    self.timers.write_control(timer, value);
                }
            }

            return;
        }

        match offset {
            Self::IE_OFFSET => {
                self.interrupts.set_interrupt_enable(value);
            }

            Self::IF_OFFSET => {
                self.interrupts.acknowledge(value);
            }

            Self::IME_OFFSET => {
                self.interrupts.set_master_enable(value);
            }

            _ => {
                let [low, high] = value.to_le_bytes();

                self.write_raw8(offset, low);

                self.write_raw8(offset.wrapping_add(1), high);
            }
        }
    }

    pub fn read32(&self, offset: u32) -> u32 {
        let offset = offset & !3;

        let low = self.read16(offset) as u32;

        let high = self.read16(offset.wrapping_add(2)) as u32;

        low | (high << 16)
    }

    pub fn write32(&mut self, offset: u32, value: u32) {
        let offset = offset & !3;

        /*
         * A word access to 0x200 covers both:
         *
         * IE at 0x200
         * IF at 0x202
         *
         * Calling write16 preserves each register's semantics,
         * including IF write-one-to-clear.
         */
        self.write16(offset, value as u16);

        self.write16(offset.wrapping_add(2), (value >> 16) as u16);
    }

    fn read_raw8(&self, offset: u32) -> u8 {
        self.raw.get(offset as usize).copied().unwrap_or(0)
    }

    fn write_raw8(&mut self, offset: u32, value: u8) {
        if let Some(byte) = self.raw.get_mut(offset as usize) {
            *byte = value;
        }
    }
}

impl Default for IoRegisters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmaRegister {
    SourceLow,
    SourceHigh,
    DestinationLow,
    DestinationHigh,
    Count,
    Control,
}

fn decode_dma_register(offset: u32) -> Option<(DmaChannelIndex, DmaRegister)> {
    if !(IoRegisters::DMA0SAD_OFFSET..=IoRegisters::DMA3CNT_H_OFFSET).contains(&offset) {
        return None;
    }

    let relative = offset - IoRegisters::DMA0SAD_OFFSET;

    let channel_number = (relative / 12) as usize;

    if channel_number >= 4 {
        return None;
    }

    let register_offset = relative % 12;

    let register = match register_offset {
        0 => DmaRegister::SourceLow,
        2 => DmaRegister::SourceHigh,
        4 => DmaRegister::DestinationLow,
        6 => DmaRegister::DestinationHigh,
        8 => DmaRegister::Count,
        10 => DmaRegister::Control,
        _ => return None,
    };

    Some((DmaChannelIndex::from_usize(channel_number)?, register))
}

fn decode_timer_register(offset: u32) -> Option<(TimerIndex, TimerRegister)> {
    if !(IoRegisters::TM0CNT_L_OFFSET..=IoRegisters::TM3CNT_H_OFFSET).contains(&offset) {
        return None;
    }

    let relative = offset - IoRegisters::TM0CNT_L_OFFSET;

    let timer_number = (relative / 4) as usize;

    let register = match relative % 4 {
        0 | 1 => TimerRegister::Counter,
        2 | 3 => TimerRegister::Control,
        _ => unreachable!(),
    };

    Some((TimerIndex::from_usize(timer_number)?, register))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerRegister {
    Counter,
    Control,
}

const fn is_affine_video_register(offset: u32) -> bool {
    matches!(
        offset,
        IoRegisters::BG0CNT_OFFSET
            | IoRegisters::BG1CNT_OFFSET
            | IoRegisters::BG2CNT_OFFSET
            | IoRegisters::BG3CNT_OFFSET
            | IoRegisters::BG0HOFS_OFFSET
            | IoRegisters::BG0VOFS_OFFSET
            | IoRegisters::BG1HOFS_OFFSET
            | IoRegisters::BG1VOFS_OFFSET
            | IoRegisters::BG2HOFS_OFFSET
            | IoRegisters::BG2VOFS_OFFSET
            | IoRegisters::BG3HOFS_OFFSET
            | IoRegisters::BG3VOFS_OFFSET
            | IoRegisters::BLDCNT_OFFSET
            | IoRegisters::BLDALPHA_OFFSET
            | IoRegisters::BLDY_OFFSET
            | IoRegisters::BG2PA_OFFSET
            | IoRegisters::BG2PB_OFFSET
            | IoRegisters::BG2PC_OFFSET
            | IoRegisters::BG2PD_OFFSET
            | IoRegisters::BG2X_L_OFFSET
            | IoRegisters::BG2X_H_OFFSET
            | IoRegisters::BG2Y_L_OFFSET
            | IoRegisters::BG2Y_H_OFFSET
            | IoRegisters::BG3PA_OFFSET
            | IoRegisters::BG3PB_OFFSET
            | IoRegisters::BG3PC_OFFSET
            | IoRegisters::BG3PD_OFFSET
            | IoRegisters::BG3X_L_OFFSET
            | IoRegisters::BG3X_H_OFFSET
            | IoRegisters::BG3Y_L_OFFSET
            | IoRegisters::BG3Y_H_OFFSET
    )
}

const fn replace_byte(original: u16, high_byte: bool, value: u8) -> u16 {
    if high_byte {
        (original & 0x00FF) | ((value as u16) << 8)
    } else {
        (original & 0xFF00) | value as u16
    }
}

#[cfg(test)]
mod tests {
    use super::IoRegisters;

    use crate::bus::{
        DmaChannelIndex, DmaTransferWidth, InterruptController, InterruptSource, Key,
        PowerStateRequest, Ppu, TimerIndex, VideoMode, WaitControl,
    };

    #[test]
    fn ie_halfword_read_write() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IE_OFFSET, 0x1234);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn ie_byte_access_is_little_endian() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::IE_OFFSET, 0x34);

        io.write8(IoRegisters::IE_OFFSET + 1, 0x12);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            0x1234 & InterruptController::SUPPORTED_MASK
        );
    }

    #[test]
    fn if_write_one_to_clear_works_for_halfword() {
        let mut io = IoRegisters::new();

        io.request_interrupt(InterruptSource::VBlank);

        io.request_interrupt(InterruptSource::Timer0);

        io.write16(IoRegisters::IF_OFFSET, InterruptSource::VBlank.mask());

        assert_eq!(
            io.read16(IoRegisters::IF_OFFSET,),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn if_write_one_to_clear_works_for_bytes() {
        let mut io = IoRegisters::new();

        io.interrupts_mut().request_mask(0x1101);

        io.write8(IoRegisters::IF_OFFSET, 0x01);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0x1100);

        io.write8(IoRegisters::IF_OFFSET + 1, 0x10);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0x0100);
    }

    #[test]
    fn ime_only_uses_bit_zero() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IME_OFFSET, 0xFFFF);

        assert_eq!(io.read16(IoRegisters::IME_OFFSET,), 1);

        assert_eq!(io.read8(IoRegisters::IME_OFFSET + 1,), 0);
    }

    #[test]
    fn word_write_at_ie_also_acknowledges_if() {
        let mut io = IoRegisters::new();

        io.request_interrupt(InterruptSource::Timer0);

        /*
         * Low halfword:
         * IE <- Timer0
         *
         * High halfword:
         * IF acknowledge Timer0
         */
        let value =
            (InterruptSource::Timer0.mask() as u32) << 16 | InterruptSource::Timer0.mask() as u32;

        io.write32(IoRegisters::IE_OFFSET, value);

        assert_eq!(
            io.read16(IoRegisters::IE_OFFSET,),
            InterruptSource::Timer0.mask()
        );

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0);
    }

    #[test]
    fn unimplemented_io_registers_use_backing_storage() {
        let mut io = IoRegisters::new();

        const UNIMPLEMENTED_OFFSET: u32 = 0x03F0;

        io.write16(UNIMPLEMENTED_OFFSET, 0xCAFE);

        assert_eq!(io.read16(UNIMPLEMENTED_OFFSET), 0xCAFE,);
    }

    #[test]
    fn reset_clears_io_and_interrupt_state() {
        let mut io = IoRegisters::new();

        io.write16(0x0100, 0xCAFE);

        io.write16(IoRegisters::IE_OFFSET, InterruptSource::Timer0.mask());

        io.write16(IoRegisters::IME_OFFSET, 1);

        io.request_interrupt(InterruptSource::Timer0);

        assert!(io.irq_line());

        io.reset();

        assert_eq!(io.read16(0x0100), 0);

        assert_eq!(io.read16(IoRegisters::IE_OFFSET,), 0);

        assert_eq!(io.read16(IoRegisters::IF_OFFSET,), 0);

        assert_eq!(io.read16(IoRegisters::IME_OFFSET,), 0);

        assert!(!io.irq_line());
    }

    #[test]
    fn timer_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFF0);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0xFFF0);

        assert_eq!(io.read16(IoRegisters::TM0CNT_H_OFFSET,), 1 << 7);
    }

    #[test]
    fn io_tick_advances_timer() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        io.tick(42);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 42);
    }

    #[test]
    fn timer_overflow_sets_if() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFFF);

        /*
         * Enable + IRQ.
         */
        io.write16(IoRegisters::TM0CNT_H_OFFSET, (1 << 7) | (1 << 6));

        io.tick(1);

        assert_eq!(
            io.read16(IoRegisters::IF_OFFSET) & InterruptSource::Timer0.mask(),
            InterruptSource::Timer0.mask()
        );
    }

    #[test]
    fn byte_writes_to_timer_reload_merge_against_reload_latch() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::TM0CNT_L_OFFSET, 0x34);

        io.write8(IoRegisters::TM0CNT_L_OFFSET + 1, 0x12);

        /*
         * Enable loads reload into the active counter.
         */
        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0x1234);
    }

    #[test]
    fn timer_irq_can_raise_irq_line() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::IE_OFFSET, InterruptSource::Timer0.mask());

        io.write16(IoRegisters::IME_OFFSET, 1);

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xFFFF);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, (1 << 7) | (1 << 6));

        assert!(!io.irq_line());

        io.tick(1);

        assert!(io.irq_line());
    }

    #[test]
    fn writing_timer_counter_updates_reload_latch_not_active_counter() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xCAFE);

        /*
         * TM0CNT_L reads the current counter.
         * Writing while disabled only updates the reload latch.
         */
        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0,);

        assert_eq!(io.timers().timer(TimerIndex::Timer0).reload(), 0xCAFE,);
    }

    #[test]
    fn enabling_timer_loads_reload_latch_into_counter() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::TM0CNT_L_OFFSET, 0xCAFE);

        io.write16(IoRegisters::TM0CNT_H_OFFSET, 1 << 7);

        assert_eq!(io.read16(IoRegisters::TM0CNT_L_OFFSET,), 0xCAFE,);
    }

    #[test]
    fn dma_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write32(IoRegisters::DMA0SAD_OFFSET, 0x0200_0100);

        io.write32(IoRegisters::DMA0DAD_OFFSET, 0x0300_0200);

        io.write16(IoRegisters::DMA0CNT_L_OFFSET, 16);

        assert_eq!(io.dma().read_source(DmaChannelIndex::Dma0), 0x0200_0100);

        assert_eq!(
            io.dma().read_destination(DmaChannelIndex::Dma0),
            0x0300_0200
        );

        assert_eq!(io.dma().read_count(DmaChannelIndex::Dma0), 16);
    }

    #[test]
    fn enabling_immediate_dma_marks_channel_pending() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DMA0CNT_L_OFFSET, 4);

        io.write16(IoRegisters::DMA0CNT_H_OFFSET, 1 << 15);

        assert!(io.dma().channel(DmaChannelIndex::Dma0).pending());
    }

    #[test]
    fn dma_word_write_can_set_count_and_control() {
        let mut io = IoRegisters::new();

        let count = 4u32;
        let control = ((1 << 15) | (1 << 10)) as u32;

        io.write32(IoRegisters::DMA0CNT_L_OFFSET, count | (control << 16));

        let request = io.dma_mut().next_pending_request().unwrap();

        assert_eq!(request.count, 4);

        assert_eq!(request.width, DmaTransferWidth::Word);
    }

    #[test]
    fn dispstat_and_vcount_are_mapped() {
        let mut io = IoRegisters::new();

        /*
         * HBlank IRQ enable, VCOUNT compare = 10.
         */
        io.write16(IoRegisters::DISPSTAT_OFFSET, (1 << 4) | (10 << 8));

        assert_eq!(
            io.read16(IoRegisters::DISPSTAT_OFFSET,) & ((1 << 4) | 0xFF00),
            (1 << 4) | (10 << 8),
        );

        io.tick(Ppu::CYCLES_PER_LINE as u32 * 10);

        assert_eq!(io.read16(IoRegisters::VCOUNT_OFFSET,), 10,);

        assert_ne!(io.read16(IoRegisters::DISPSTAT_OFFSET,) & (1 << 2), 0,);
    }

    #[test]
    fn hblank_sets_dispstat_status_bit() {
        let mut io = IoRegisters::new();

        io.tick(Ppu::HDRAW_CYCLES as u32);

        assert_ne!(io.read16(IoRegisters::DISPSTAT_OFFSET,) & (1 << 1), 0,);
    }

    #[test]
    fn hblank_event_queues_hblank_dma() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DMA0CNT_L_OFFSET, 1);

        io.write16(IoRegisters::DMA0CNT_H_OFFSET, (0b10 << 12) | (1 << 15));

        assert!(io.dma_mut().next_pending_request().is_none());

        io.tick(Ppu::HDRAW_CYCLES as u32);

        let request = io.dma_mut().next_pending_request().unwrap();

        assert_eq!(request.channel, DmaChannelIndex::Dma0,);
    }

    #[test]
    fn vblank_event_queues_vblank_dma() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DMA1CNT_L_OFFSET, 1);

        io.write16(IoRegisters::DMA1CNT_H_OFFSET, (0b01 << 12) | (1 << 15));

        io.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        let request = io.dma_mut().next_pending_request().unwrap();

        assert_eq!(request.channel, DmaChannelIndex::Dma1,);
    }

    #[test]
    fn ppu_hblank_irq_sets_if() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DISPSTAT_OFFSET, 1 << 4);

        io.tick(Ppu::HDRAW_CYCLES as u32);

        assert_ne!(
            io.read16(IoRegisters::IF_OFFSET) & InterruptSource::HBlank.mask(),
            0,
        );
    }

    #[test]
    fn keyinput_is_mapped_and_active_low() {
        let mut io = IoRegisters::new();

        assert_eq!(io.read16(IoRegisters::KEYINPUT_OFFSET,), 0x03FF,);

        io.set_key(Key::A, true);

        assert_eq!(io.read16(IoRegisters::KEYINPUT_OFFSET,) & Key::A.mask(), 0,);
    }

    #[test]
    fn keycnt_can_request_keypad_interrupt() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::KEYCNT_OFFSET, Key::Start.mask() | (1 << 14));

        io.set_key(Key::Start, true);

        assert_ne!(
            io.read16(IoRegisters::IF_OFFSET) & InterruptSource::Keypad.mask(),
            0,
        );
    }

    #[test]
    fn keyinput_is_read_only() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::KEYINPUT_OFFSET, 0);

        assert_eq!(io.read16(IoRegisters::KEYINPUT_OFFSET,), 0x03FF,);
    }

    #[test]
    fn byte_write_to_haltcnt_requests_halt() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::HALTCNT_OFFSET, 0);

        assert_eq!(io.take_power_request(), Some(PowerStateRequest::Halt),);
    }

    #[test]
    fn haltcnt_bit_seven_requests_stop() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::HALTCNT_OFFSET, 0x80);

        assert_eq!(io.take_power_request(), Some(PowerStateRequest::Stop),);
    }

    #[test]
    fn postflg_is_separate_from_haltcnt() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::POSTFLG_OFFSET, 1);

        assert_eq!(io.read8(IoRegisters::POSTFLG_OFFSET,), 1,);

        assert_eq!(io.take_power_request(), None,);
    }

    #[test]
    fn postflg_and_haltcnt_do_not_use_raw_backing_storage() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::POSTFLG_OFFSET, 0xCAFE);

        /*
         * Low byte = 0xFE.
         * POSTFLG only retains bit zero, so result is zero.
         */
        assert_eq!(io.read8(IoRegisters::POSTFLG_OFFSET,), 0,);

        /*
         * High byte = 0xCA, bit 7 is set,
         * therefore this requests STOP.
         */
        assert_eq!(io.take_power_request(), Some(PowerStateRequest::Stop),);

        /*
         * HALTCNT is write-only, so the high read byte is zero.
         */
        assert_eq!(io.read16(IoRegisters::POSTFLG_OFFSET,), 0,);
    }

    #[test]
    fn waitcnt_is_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::WAITCNT_OFFSET, 0x4317);

        assert_eq!(
            io.read16(IoRegisters::WAITCNT_OFFSET,),
            0x4317 & WaitControl::WRITABLE_MASK,
        );
    }

    #[test]
    fn waitcnt_byte_access_is_little_endian() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::WAITCNT_OFFSET, 0x17);

        io.write8(IoRegisters::WAITCNT_OFFSET + 1, 0x43);

        assert_eq!(
            io.read16(IoRegisters::WAITCNT_OFFSET,),
            0x4317 & WaitControl::WRITABLE_MASK,
        );
    }

    #[test]
    fn waitcnt_reset_value_is_zero() {
        let io = IoRegisters::new();

        assert_eq!(
            io.read16(IoRegisters::WAITCNT_OFFSET,),
            WaitControl::RESET_VALUE,
        );
    }

    #[test]
    fn dispcnt_is_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::DISPCNT_OFFSET, 3 | (1 << 10));

        assert_eq!(io.read16(IoRegisters::DISPCNT_OFFSET,), 3 | (1 << 10),);

        assert_eq!(io.video().display_control().mode(), VideoMode::Mode3,);

        assert!(io.video().display_control().bg2_enabled(),);
    }

    #[test]
    fn dispcnt_byte_access_is_little_endian() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::DISPCNT_OFFSET, 0x83);

        io.write8(IoRegisters::DISPCNT_OFFSET + 1, 0x04);

        assert_eq!(io.read16(IoRegisters::DISPCNT_OFFSET,), 0x0483,);
    }

    #[test]
    fn io_tick_returns_completed_scanlines() {
        let mut io = IoRegisters::new();

        let result = io.tick(Ppu::HDRAW_CYCLES as u32);

        assert!(result.completed_visible_lines.contains(0),);
    }
    #[test]
    fn affine_background_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::BG2CNT_OFFSET, (1 << 8) | (1 << 13));
        io.write16(IoRegisters::BG2PA_OFFSET, 0x0100);
        io.write16(IoRegisters::BG2PB_OFFSET, 0xFF80);
        io.write16(IoRegisters::BG2X_L_OFFSET, 0xFF00);
        io.write16(IoRegisters::BG2X_H_OFFSET, 0x0FFF);

        assert_eq!(io.read16(IoRegisters::BG2CNT_OFFSET), (1 << 8) | (1 << 13));
        assert_eq!(io.read16(IoRegisters::BG2PA_OFFSET), 0x0100);
        assert_eq!(io.read16(IoRegisters::BG2PB_OFFSET), 0xFF80);
        assert_eq!(io.read16(IoRegisters::BG2X_L_OFFSET), 0xFF00);
        assert_eq!(io.read16(IoRegisters::BG2X_H_OFFSET) & 0x0FFF, 0x0FFF);
    }

    #[test]
    fn byte_writes_merge_affine_registers() {
        let mut io = IoRegisters::new();

        io.write8(IoRegisters::BG2PA_OFFSET, 0x34);
        io.write8(IoRegisters::BG2PA_OFFSET + 1, 0x12);

        assert_eq!(io.read16(IoRegisters::BG2PA_OFFSET), 0x1234);
    }
    #[test]
    fn mode0_background_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::BG0CNT_OFFSET, 0x1234);
        io.write16(IoRegisters::BG1CNT_OFFSET, 0x5678);
        io.write16(IoRegisters::BG0HOFS_OFFSET, 0x03FF);
        io.write16(IoRegisters::BG0VOFS_OFFSET, 0x0123);

        assert_eq!(io.read16(IoRegisters::BG0CNT_OFFSET), 0x1234);
        assert_eq!(io.read16(IoRegisters::BG1CNT_OFFSET), 0x5678);
        assert_eq!(io.read16(IoRegisters::BG0HOFS_OFFSET), 0x01FF);
        assert_eq!(io.read16(IoRegisters::BG0VOFS_OFFSET), 0x0123);
    }

    #[test]
    fn blend_registers_are_mapped() {
        let mut io = IoRegisters::new();

        io.write16(IoRegisters::BLDCNT_OFFSET, 0x3FFF);
        io.write16(IoRegisters::BLDALPHA_OFFSET, 0x1F1F);
        io.write16(IoRegisters::BLDY_OFFSET, 0x001F);

        assert_eq!(io.read16(IoRegisters::BLDCNT_OFFSET), 0x3FFF);
        assert_eq!(io.read16(IoRegisters::BLDALPHA_OFFSET), 0x1F1F);
        assert_eq!(io.read16(IoRegisters::BLDY_OFFSET), 0x001F);
    }
}
