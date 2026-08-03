use super::InterruptSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispStat {
    vblank_irq_enabled: bool,
    hblank_irq_enabled: bool,
    vcounter_irq_enabled: bool,
    vcount_setting: u8,
}

impl DispStat {
    pub const fn new() -> Self {
        Self {
            vblank_irq_enabled: false,
            hblank_irq_enabled: false,
            vcounter_irq_enabled: false,
            vcount_setting: 0,
        }
    }

    pub const fn vblank_irq_enabled(self) -> bool {
        self.vblank_irq_enabled
    }

    pub const fn hblank_irq_enabled(self) -> bool {
        self.hblank_irq_enabled
    }

    pub const fn vcounter_irq_enabled(self) -> bool {
        self.vcounter_irq_enabled
    }

    pub const fn vcount_setting(self) -> u8 {
        self.vcount_setting
    }

    pub fn write(&mut self, value: u16) {
        /*
         * DISPSTAT writable fields:
         *
         * bit 3     VBlank IRQ enable
         * bit 4     HBlank IRQ enable
         * bit 5     VCounter IRQ enable
         * bits 8-15 VCOUNT comparison value
         *
         * Status bits 0-2 are read-only.
         */
        self.vblank_irq_enabled = value & (1 << 3) != 0;

        self.hblank_irq_enabled = value & (1 << 4) != 0;

        self.vcounter_irq_enabled = value & (1 << 5) != 0;

        self.vcount_setting = (value >> 8) as u8;
    }

    pub const fn raw(self, in_vblank: bool, in_hblank: bool, vcount_match: bool) -> u16 {
        let mut value = 0u16;

        if in_vblank {
            value |= 1 << 0;
        }

        if in_hblank {
            value |= 1 << 1;
        }

        if vcount_match {
            value |= 1 << 2;
        }

        if self.vblank_irq_enabled {
            value |= 1 << 3;
        }

        if self.hblank_irq_enabled {
            value |= 1 << 4;
        }

        if self.vcounter_irq_enabled {
            value |= 1 << 5;
        }

        value | ((self.vcount_setting as u16) << 8)
    }
}

impl Default for DispStat {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PpuTickResult {
    pub hblank_starts: u32,
    pub vblank_starts: u32,
    pub vcount_matches: u32,
    pub new_frames: u32,

    /*
     * InterruptSource mask to merge into IF.
     */
    pub interrupt_requests: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ppu {
    /*
     * CPU cycles elapsed within the current scanline.
     *
     * Range: 0..1232.
     */
    line_cycle: u16,

    /*
     * Current scanline.
     *
     * Range: 0..227.
     */
    vcount: u16,

    in_hblank: bool,
    in_vblank: bool,
    vcount_match: bool,

    dispstat: DispStat,
}

impl Ppu {
    pub const HDRAW_CYCLES: u16 = 960;
    pub const HBLANK_CYCLES: u16 = 272;
    pub const CYCLES_PER_LINE: u16 = Self::HDRAW_CYCLES + Self::HBLANK_CYCLES;

    pub const VISIBLE_LINES: u16 = 160;
    pub const TOTAL_LINES: u16 = 228;

    pub const fn new() -> Self {
        Self {
            line_cycle: 0,
            vcount: 0,

            in_hblank: false,
            in_vblank: false,

            /*
             * Default DISPSTAT comparison is zero and VCOUNT starts
             * at zero.
             */
            vcount_match: true,

            dispstat: DispStat::new(),
        }
    }

    pub const fn line_cycle(&self) -> u16 {
        self.line_cycle
    }

    pub const fn vcount(&self) -> u16 {
        self.vcount
    }

    pub const fn in_hblank(&self) -> bool {
        self.in_hblank
    }

    pub const fn in_vblank(&self) -> bool {
        self.in_vblank
    }

    pub const fn vcount_match(&self) -> bool {
        self.vcount_match
    }

    pub const fn dispstat(&self) -> DispStat {
        self.dispstat
    }

    pub const fn read_dispstat(&self) -> u16 {
        self.dispstat
            .raw(self.in_vblank, self.in_hblank, self.vcount_match)
    }

    pub fn write_dispstat(&mut self, value: u16) {
        self.dispstat.write(value);

        /*
         * Update the status bit immediately. We deliberately do not
         * generate an IRQ merely because software changed the compare
         * value to the current scanline.
         */
        self.vcount_match = self.vcount as u8 == self.dispstat.vcount_setting();
    }

    pub fn tick(&mut self, cycles: u32) -> PpuTickResult {
        let mut remaining = cycles;
        let mut result = PpuTickResult::default();

        while remaining > 0 {
            let next_boundary = if self.in_hblank {
                Self::CYCLES_PER_LINE
            } else {
                Self::HDRAW_CYCLES
            };

            let cycles_until_boundary = next_boundary - self.line_cycle;

            if remaining < cycles_until_boundary as u32 {
                self.line_cycle += remaining as u16;
                break;
            }

            self.line_cycle = next_boundary;
            remaining -= cycles_until_boundary as u32;

            if !self.in_hblank {
                self.enter_hblank(&mut result);
            } else {
                self.finish_scanline(&mut result);
            }
        }

        result
    }

    fn enter_hblank(&mut self, result: &mut PpuTickResult) {
        self.in_hblank = true;
        result.hblank_starts += 1;

        if self.dispstat.hblank_irq_enabled() {
            result.interrupt_requests |= InterruptSource::HBlank.mask();
        }
    }

    fn finish_scanline(&mut self, result: &mut PpuTickResult) {
        self.line_cycle = 0;
        self.in_hblank = false;

        self.vcount += 1;

        if self.vcount == Self::VISIBLE_LINES {
            self.in_vblank = true;
            result.vblank_starts += 1;

            if self.dispstat.vblank_irq_enabled() {
                result.interrupt_requests |= InterruptSource::VBlank.mask();
            }
        }

        if self.vcount == Self::TOTAL_LINES {
            self.vcount = 0;
            self.in_vblank = false;
            result.new_frames += 1;
        }

        let previous_match = self.vcount_match;

        self.vcount_match = self.vcount as u8 == self.dispstat.vcount_setting();

        /*
         * VCounter IRQ is edge-triggered when the comparison becomes
         * true at the beginning of a scanline.
         */
        if self.vcount_match && !previous_match {
            result.vcount_matches += 1;

            if self.dispstat.vcounter_irq_enabled() {
                result.interrupt_requests |= InterruptSource::VCounterMatch.mask();
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Ppu;

    use crate::bus::InterruptSource;

    #[test]
    fn starts_on_first_visible_scanline() {
        let ppu = Ppu::new();

        assert_eq!(ppu.vcount(), 0);
        assert_eq!(ppu.line_cycle(), 0);
        assert!(!ppu.in_hblank());
        assert!(!ppu.in_vblank());
        assert!(ppu.vcount_match());
    }

    #[test]
    fn enters_hblank_after_960_cycles() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::HDRAW_CYCLES as u32);

        assert!(ppu.in_hblank());
        assert_eq!(ppu.vcount(), 0);
        assert_eq!(result.hblank_starts, 1);
    }

    #[test]
    fn advances_scanline_after_1232_cycles() {
        let mut ppu = Ppu::new();

        ppu.tick(Ppu::CYCLES_PER_LINE as u32);

        assert_eq!(ppu.vcount(), 1);
        assert_eq!(ppu.line_cycle(), 0);
        assert!(!ppu.in_hblank());
    }

    #[test]
    fn enters_vblank_at_scanline_160() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        assert_eq!(ppu.vcount(), 160);
        assert!(ppu.in_vblank());
        assert_eq!(result.vblank_starts, 1);
    }

    #[test]
    fn wraps_after_scanline_227() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::TOTAL_LINES as u32);

        assert_eq!(ppu.vcount(), 0);
        assert!(!ppu.in_vblank());
        assert_eq!(result.new_frames, 1);
    }

    #[test]
    fn hblank_irq_is_requested_when_enabled() {
        let mut ppu = Ppu::new();

        ppu.write_dispstat(1 << 4);

        let result = ppu.tick(Ppu::HDRAW_CYCLES as u32);

        assert_ne!(
            result.interrupt_requests & InterruptSource::HBlank.mask(),
            0,
        );
    }

    #[test]
    fn vblank_irq_is_requested_when_enabled() {
        let mut ppu = Ppu::new();

        ppu.write_dispstat(1 << 3);

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * Ppu::VISIBLE_LINES as u32);

        assert_ne!(
            result.interrupt_requests & InterruptSource::VBlank.mask(),
            0,
        );
    }

    #[test]
    fn vcounter_match_requests_interrupt() {
        let mut ppu = Ppu::new();

        /*
         * Compare VCOUNT with scanline 10 and enable its IRQ.
         */
        ppu.write_dispstat((10 << 8) | (1 << 5));

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * 10);

        assert_eq!(ppu.vcount(), 10);
        assert!(ppu.vcount_match());

        assert_ne!(
            result.interrupt_requests & InterruptSource::VCounterMatch.mask(),
            0,
        );
    }

    #[test]
    fn tick_can_cross_multiple_scanlines() {
        let mut ppu = Ppu::new();

        let result = ppu.tick(Ppu::CYCLES_PER_LINE as u32 * 3 + Ppu::HDRAW_CYCLES as u32);

        assert_eq!(ppu.vcount(), 3);
        assert!(ppu.in_hblank());
        assert_eq!(result.hblank_starts, 4);
    }
}
