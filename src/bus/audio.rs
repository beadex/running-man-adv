use std::collections::VecDeque;

pub const AUDIO_OUTPUT_RATE: u32 = 48_000;
const GBA_CLOCK_HZ: u64 = 16_777_216;
const FIFO_CAPACITY: usize = 32;
const FIFO_DMA_THRESHOLD: usize = 16;
const MAX_OUTPUT_SAMPLES: usize = AUDIO_OUTPUT_RATE as usize / 6 * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectSoundFifo {
    A = 0,
    B = 1,
}

impl DirectSoundFifo {
    pub const fn as_usize(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioTickResult {
    pub fifo_dma_requests: [bool; 2],
}

#[derive(Debug, Clone)]
struct DirectSoundChannel {
    fifo: VecDeque<i8>,
    current_sample: i8,
}

impl DirectSoundChannel {
    fn new() -> Self {
        Self {
            fifo: VecDeque::with_capacity(FIFO_CAPACITY),
            current_sample: 0,
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.fifo.len() == FIFO_CAPACITY {
                break;
            }

            self.fifo.push_back(*byte as i8);
        }
    }

    fn clock(&mut self, count: u64) {
        if count == 0 {
            return;
        }

        let consumed = count.min(self.fifo.len() as u64) as usize;

        for _ in 0..consumed {
            self.current_sample = self.fifo.pop_front().expect("FIFO length was checked");
        }

        if count > consumed as u64 {
            self.current_sample = 0;
        }
    }

    fn reset(&mut self) {
        self.fifo.clear();
        self.current_sample = 0;
    }
}

#[derive(Debug, Clone)]
pub struct Audio {
    sound_control_high: u16,
    sound_control_x: u16,
    sound_bias: u16,
    channels: [DirectSoundChannel; 2],
    output_enabled: bool,
    output_phase: u64,
    output_samples: VecDeque<i16>,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            sound_control_high: 0,
            sound_control_x: 0,
            sound_bias: 0x0200,
            channels: [DirectSoundChannel::new(), DirectSoundChannel::new()],
            output_enabled: false,
            output_phase: 0,
            output_samples: VecDeque::with_capacity(MAX_OUTPUT_SAMPLES),
        }
    }

    pub const fn read_sound_control_high(&self) -> u16 {
        self.sound_control_high
    }

    pub fn write_sound_control_high(&mut self, value: u16) {
        const WRITABLE_MASK: u16 = 0xFF0F;
        const FIFO_A_RESET: u16 = 1 << 11;
        const FIFO_B_RESET: u16 = 1 << 15;

        if value & FIFO_A_RESET != 0 {
            self.channels[DirectSoundFifo::A.as_usize()].reset();
        }

        if value & FIFO_B_RESET != 0 {
            self.channels[DirectSoundFifo::B.as_usize()].reset();
        }

        /* FIFO reset bits are write-only and self-clear. */
        self.sound_control_high = value & WRITABLE_MASK & !(FIFO_A_RESET | FIFO_B_RESET);
    }

    pub const fn read_sound_control_x(&self) -> u16 {
        self.sound_control_x & (1 << 7)
    }

    pub fn write_sound_control_x(&mut self, value: u16) {
        let was_enabled = self.master_enabled();
        self.sound_control_x = value & (1 << 7);

        if was_enabled && !self.master_enabled() {
            for channel in &mut self.channels {
                channel.reset();
            }

            self.sound_control_high = 0;
        }
    }

    pub const fn read_sound_bias(&self) -> u16 {
        self.sound_bias
    }

    pub fn write_sound_bias(&mut self, value: u16) {
        self.sound_bias = value & 0xC3FE;
    }

    pub fn write_fifo16(&mut self, fifo: DirectSoundFifo, value: u16) {
        self.channels[fifo.as_usize()].push_bytes(&value.to_le_bytes());
    }

    pub fn write_fifo8(&mut self, fifo: DirectSoundFifo, value: u8) {
        self.channels[fifo.as_usize()].push_bytes(&[value]);
    }

    pub fn set_output_enabled(&mut self, enabled: bool) {
        self.output_enabled = enabled;
        self.output_phase = 0;

        if !enabled {
            self.output_samples.clear();
        }
    }

    pub fn drain_output_samples(&mut self, destination: &mut Vec<i16>) {
        destination.extend(self.output_samples.drain(..));
    }

    pub fn fifo_level(&self, fifo: DirectSoundFifo) -> usize {
        self.channels[fifo.as_usize()].fifo.len()
    }

    pub fn tick(&mut self, cycles: u32, timer_overflows: [u64; 4]) -> AudioTickResult {
        let mut fifo_dma_requests = [false; 2];

        if self.master_enabled() {
            for fifo in [DirectSoundFifo::A, DirectSoundFifo::B] {
                let channel_index = fifo.as_usize();
                let timer_index = self.timer_index(fifo);
                let overflow_count = timer_overflows[timer_index];

                self.channels[channel_index].clock(overflow_count);

                fifo_dma_requests[channel_index] = overflow_count != 0
                    && self.channels[channel_index].fifo.len() <= FIFO_DMA_THRESHOLD;
            }
        }

        if self.output_enabled {
            self.output_phase += cycles as u64 * AUDIO_OUTPUT_RATE as u64;

            while self.output_phase >= GBA_CLOCK_HZ {
                self.output_phase -= GBA_CLOCK_HZ;
                let [left, right] = self.mix_sample();

                while self.output_samples.len() + 2 > MAX_OUTPUT_SAMPLES {
                    self.output_samples.pop_front();
                    self.output_samples.pop_front();
                }

                self.output_samples.push_back(left);
                self.output_samples.push_back(right);
            }
        }

        AudioTickResult { fifo_dma_requests }
    }

    fn mix_sample(&self) -> [i16; 2] {
        if !self.master_enabled() {
            return [0, 0];
        }

        let mut left = 0i32;
        let mut right = 0i32;

        for fifo in [DirectSoundFifo::A, DirectSoundFifo::B] {
            let index = fifo.as_usize();
            let sample = self.channels[index].current_sample as i32;
            let full_volume = match fifo {
                DirectSoundFifo::A => self.sound_control_high & (1 << 2) != 0,
                DirectSoundFifo::B => self.sound_control_high & (1 << 3) != 0,
            };
            let scaled = sample * if full_volume { 128 } else { 64 };

            let (right_enabled, left_enabled) = match fifo {
                DirectSoundFifo::A => (
                    self.sound_control_high & (1 << 8) != 0,
                    self.sound_control_high & (1 << 9) != 0,
                ),
                DirectSoundFifo::B => (
                    self.sound_control_high & (1 << 12) != 0,
                    self.sound_control_high & (1 << 13) != 0,
                ),
            };

            if left_enabled {
                left += scaled;
            }

            if right_enabled {
                right += scaled;
            }
        }

        [
            left.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            right.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        ]
    }

    const fn timer_index(&self, fifo: DirectSoundFifo) -> usize {
        match fifo {
            DirectSoundFifo::A => ((self.sound_control_high >> 10) & 1) as usize,
            DirectSoundFifo::B => ((self.sound_control_high >> 14) & 1) as usize,
        }
    }

    const fn master_enabled(&self) -> bool {
        self.sound_control_x & (1 << 7) != 0
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Audio, DirectSoundFifo, GBA_CLOCK_HZ};

    #[test]
    fn fifo_word_write_is_little_endian_and_timer_consumes_samples() {
        let mut audio = Audio::new();
        audio.write_sound_control_x(1 << 7);
        audio.write_fifo16(DirectSoundFifo::A, 0xFF80);
        audio.write_fifo16(DirectSoundFifo::A, 0x7F02);

        assert_eq!(audio.fifo_level(DirectSoundFifo::A), 4);

        audio.tick(1, [1, 0, 0, 0]);

        assert_eq!(audio.channels[0].current_sample, -128);
        assert_eq!(audio.fifo_level(DirectSoundFifo::A), 3);
    }

    #[test]
    fn selected_timer_clocks_fifo_and_requests_dma_at_half_empty() {
        let mut audio = Audio::new();
        audio.write_sound_control_x(1 << 7);
        audio.write_sound_control_high(1 << 10);
        audio.write_fifo16(DirectSoundFifo::A, 0x0201);
        audio.write_fifo16(DirectSoundFifo::A, 0x0403);

        assert!(!audio.tick(1, [1, 0, 0, 0]).fifo_dma_requests[0]);

        let result = audio.tick(1, [0, 1, 0, 0]);

        assert!(result.fifo_dma_requests[0]);
        assert_eq!(audio.channels[0].current_sample, 1);
    }

    #[test]
    fn fifo_reset_bit_clears_data_and_self_clears() {
        let mut audio = Audio::new();
        audio.write_sound_control_x(1 << 7);
        audio.write_fifo16(DirectSoundFifo::B, 0x0201);
        audio.write_fifo16(DirectSoundFifo::B, 0x0403);
        audio.write_sound_control_high((1 << 15) | (1 << 13));

        assert_eq!(audio.fifo_level(DirectSoundFifo::B), 0);
        assert_eq!(audio.read_sound_control_high(), 1 << 13);
    }

    #[test]
    fn disabled_master_stops_fifo_clock_and_dma_requests() {
        let mut audio = Audio::new();
        audio.write_fifo16(DirectSoundFifo::A, 0x0201);

        let result = audio.tick(1, [1, 0, 0, 0]);

        assert_eq!(audio.fifo_level(DirectSoundFifo::A), 2);
        assert!(!result.fifo_dma_requests[0]);
    }

    #[test]
    fn output_is_only_generated_when_frontend_enables_it() {
        let mut audio = Audio::new();
        let mut samples = Vec::new();

        audio.tick(GBA_CLOCK_HZ as u32 / 60, [0; 4]);
        audio.drain_output_samples(&mut samples);
        assert!(samples.is_empty());

        audio.set_output_enabled(true);
        audio.tick(GBA_CLOCK_HZ as u32 / 60, [0; 4]);
        audio.drain_output_samples(&mut samples);

        assert!((1_590..=1_610).contains(&samples.len()));
    }
}
