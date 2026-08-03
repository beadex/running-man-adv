pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 160;
pub const FRAMEBUFFER_PIXEL_COUNT: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

pub type Framebuffer = [u32; FRAMEBUFFER_PIXEL_COUNT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Invalid6,
    Invalid7,
}

impl VideoMode {
    pub const fn from_bits(bits: u16) -> Self {
        match bits & 0b111 {
            0 => Self::Mode0,
            1 => Self::Mode1,
            2 => Self::Mode2,
            3 => Self::Mode3,
            4 => Self::Mode4,
            5 => Self::Mode5,
            6 => Self::Invalid6,
            7 => Self::Invalid7,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayControl {
    raw: u16,
}

impl DisplayControl {
    pub const MODE_MASK: u16 = 0b111;
    pub const CGB_MODE_MASK: u16 = 1 << 3;
    pub const PAGE_SELECT_MASK: u16 = 1 << 4;
    pub const HBLANK_INTERVAL_FREE_MASK: u16 = 1 << 5;
    pub const OBJ_MAPPING_1D_MASK: u16 = 1 << 6;
    pub const FORCED_BLANK_MASK: u16 = 1 << 7;

    pub const BG0_ENABLE_MASK: u16 = 1 << 8;
    pub const BG1_ENABLE_MASK: u16 = 1 << 9;
    pub const BG2_ENABLE_MASK: u16 = 1 << 10;
    pub const BG3_ENABLE_MASK: u16 = 1 << 11;
    pub const OBJ_ENABLE_MASK: u16 = 1 << 12;

    pub const WINDOW0_ENABLE_MASK: u16 = 1 << 13;
    pub const WINDOW1_ENABLE_MASK: u16 = 1 << 14;
    pub const OBJ_WINDOW_ENABLE_MASK: u16 = 1 << 15;

    /*
     * Bit 3 is read-only on GBA hardware. We mask it out for now.
     * All other DISPCNT bits are retained.
     */
    pub const WRITABLE_MASK: u16 = 0xFFF7;

    pub const RESET_VALUE: u16 = 0;

    pub const fn new() -> Self {
        Self {
            raw: Self::RESET_VALUE,
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }

    pub fn write(&mut self, value: u16) {
        self.raw = value & Self::WRITABLE_MASK;
    }

    pub const fn mode(self) -> VideoMode {
        VideoMode::from_bits(self.raw)
    }

    pub const fn page_selected(self) -> bool {
        self.raw & Self::PAGE_SELECT_MASK != 0
    }

    pub const fn forced_blank(self) -> bool {
        self.raw & Self::FORCED_BLANK_MASK != 0
    }

    pub const fn bg2_enabled(self) -> bool {
        self.raw & Self::BG2_ENABLE_MASK != 0
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for DisplayControl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Video {
    display_control: DisplayControl,
    framebuffer: Box<Framebuffer>,

    /*
     * Set when scanline 160 begins.
     *
     * The frontend consumes this through take_frame_ready().
     */
    frame_ready: bool,

    frame_number: u64,
}

impl Video {
    pub const FORCED_BLANK_PIXEL: u32 = 0xFFFF_FFFF;

    /*
     * Until a mode is implemented, render black instead of exposing
     * stale pixels from a previous mode.
     */
    pub const UNIMPLEMENTED_MODE_PIXEL: u32 = 0xFF00_0000;

    pub fn new() -> Self {
        Self {
            display_control: DisplayControl::new(),

            framebuffer: Box::new([Self::UNIMPLEMENTED_MODE_PIXEL; FRAMEBUFFER_PIXEL_COUNT]),

            frame_ready: false,
            frame_number: 0,
        }
    }

    pub const fn display_control(&self) -> DisplayControl {
        self.display_control
    }

    pub const fn read_display_control(&self) -> u16 {
        self.display_control.raw()
    }

    pub fn write_display_control(&mut self, value: u16) {
        self.display_control.write(value);
    }

    pub fn write_display_control_low(&mut self, value: u8) {
        let current = self.display_control.raw();

        let updated = (current & 0xFF00) | value as u16;

        self.display_control.write(updated);
    }

    pub fn write_display_control_high(&mut self, value: u8) {
        let current = self.display_control.raw();

        let updated = (current & 0x00FF) | ((value as u16) << 8);

        self.display_control.write(updated);
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.framebuffer.as_slice()
    }

    pub fn framebuffer_mut(&mut self) -> &mut [u32] {
        self.framebuffer.as_mut_slice()
    }

    pub const fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    pub fn take_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    pub const fn frame_number(&self) -> u64 {
        self.frame_number
    }

    pub fn mark_frame_ready(&mut self) {
        self.frame_ready = true;

        self.frame_number = self.frame_number.wrapping_add(1);
    }

    pub fn render_scanline(&mut self, line: u16, vram: &[u8]) {
        let line = line as usize;

        if line >= SCREEN_HEIGHT {
            return;
        }

        if self.display_control.forced_blank() {
            self.fill_scanline(line, Self::FORCED_BLANK_PIXEL);

            return;
        }

        match self.display_control.mode() {
            VideoMode::Mode3 => {
                self.render_mode3_scanline(line, vram);
            }

            _ => {
                self.fill_scanline(line, Self::UNIMPLEMENTED_MODE_PIXEL);
            }
        }
    }

    fn render_mode3_scanline(&mut self, line: usize, vram: &[u8]) {
        /*
         * Mode 3:
         *
         * 240 × 160
         * 16-bit BGR555
         * one framebuffer at VRAM offset zero
         */
        let source_line_start = line * SCREEN_WIDTH * 2;

        let destination_line_start = line * SCREEN_WIDTH;

        for x in 0..SCREEN_WIDTH {
            let source_offset = source_line_start + x * 2;

            let low = vram.get(source_offset).copied().unwrap_or(0);

            let high = vram.get(source_offset + 1).copied().unwrap_or(0);

            let color = u16::from_le_bytes([low, high]);

            self.framebuffer[destination_line_start + x] = bgr555_to_rgba8888(color);
        }
    }

    fn fill_scanline(&mut self, line: usize, color: u32) {
        let start = line * SCREEN_WIDTH;

        let end = start + SCREEN_WIDTH;

        self.framebuffer[start..end].fill(color);
    }

    pub fn clear_framebuffer(&mut self, color: u32) {
        self.framebuffer.fill(color);
    }

    pub fn reset(&mut self) {
        self.display_control.reset();

        self.framebuffer.fill(Self::UNIMPLEMENTED_MODE_PIXEL);

        self.frame_ready = false;
        self.frame_number = 0;
    }
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts GBA BGR555 into packed RGBA8888:
///
/// ```text
/// u32 bits:
/// 31..24 = alpha
/// 23..16 = red
/// 15..8  = green
/// 7..0   = blue
/// ```
///
/// This representation displays correctly as `0xAARRGGBB`.
pub const fn bgr555_to_rgba8888(color: u16) -> u32 {
    let red5 = (color & 0x001F) as u32;

    let green5 = ((color >> 5) & 0x001F) as u32;

    let blue5 = ((color >> 10) & 0x001F) as u32;

    /*
     * Expand five bits to eight by repeating the top bits:
     *
     * abcde -> abcdeabc
     */
    let red8 = (red5 << 3) | (red5 >> 2);

    let green8 = (green5 << 3) | (green5 >> 2);

    let blue8 = (blue5 << 3) | (blue5 >> 2);

    0xFF00_0000 | (red8 << 16) | (green8 << 8) | blue8
}

#[cfg(test)]
mod tests {
    use super::{
        DisplayControl, SCREEN_HEIGHT, SCREEN_WIDTH, Video, VideoMode, bgr555_to_rgba8888,
    };

    #[test]
    fn display_control_decodes_mode() {
        let mut control = DisplayControl::new();

        control.write(3);

        assert_eq!(control.mode(), VideoMode::Mode3,);
    }

    #[test]
    fn converts_bgr555_primary_colors() {
        assert_eq!(bgr555_to_rgba8888(0x001F), 0xFFFF_0000,);

        assert_eq!(bgr555_to_rgba8888(0x03E0), 0xFF00_FF00,);

        assert_eq!(bgr555_to_rgba8888(0x7C00), 0xFF00_00FF,);

        assert_eq!(bgr555_to_rgba8888(0x7FFF), 0xFFFF_FFFF,);

        assert_eq!(bgr555_to_rgba8888(0x0000), 0xFF00_0000,);
    }

    #[test]
    fn mode3_renders_scanline() {
        let mut video = Video::new();

        video.write_display_control(3);

        let mut vram = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 2];

        vram[0..2].copy_from_slice(&0x001Fu16.to_le_bytes());

        vram[2..4].copy_from_slice(&0x03E0u16.to_le_bytes());

        vram[4..6].copy_from_slice(&0x7C00u16.to_le_bytes());

        video.render_scanline(0, &vram);

        assert_eq!(video.framebuffer()[0], 0xFFFF_0000,);

        assert_eq!(video.framebuffer()[1], 0xFF00_FF00,);

        assert_eq!(video.framebuffer()[2], 0xFF00_00FF,);
    }

    #[test]
    fn mode3_uses_correct_scanline_offset() {
        let mut video = Video::new();

        video.write_display_control(3);

        let mut vram = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 2];

        let line = 10usize;
        let x = 20usize;

        let vram_offset = (line * SCREEN_WIDTH + x) * 2;

        vram[vram_offset..vram_offset + 2].copy_from_slice(&0x7FFFu16.to_le_bytes());

        video.render_scanline(line as u16, &vram);

        assert_eq!(video.framebuffer()[line * SCREEN_WIDTH + x], 0xFFFF_FFFF,);
    }

    #[test]
    fn forced_blank_outputs_white() {
        let mut video = Video::new();

        video.write_display_control(3 | (1 << 7));

        let vram = vec![0u8; SCREEN_WIDTH * 2];

        video.render_scanline(0, &vram);

        assert!(
            video.framebuffer()[0..SCREEN_WIDTH]
                .iter()
                .all(|&pixel| { pixel == Video::FORCED_BLANK_PIXEL },),
        );
    }

    #[test]
    fn frame_ready_is_consumable() {
        let mut video = Video::new();

        assert!(!video.frame_ready());

        video.mark_frame_ready();

        assert!(video.frame_ready());
        assert!(video.take_frame_ready());
        assert!(!video.frame_ready());
        assert!(!video.take_frame_ready());

        assert_eq!(video.frame_number(), 1);
    }
}
