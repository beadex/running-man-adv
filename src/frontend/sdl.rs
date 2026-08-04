use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use sdl3::{
    audio::{AudioFormat, AudioSpec},
    event::Event,
    keyboard::Keycode,
    pixels::{Color, PixelFormat},
    render::ScaleMode,
};

use crate::{
    bus::{AUDIO_OUTPUT_RATE, Key, SCREEN_HEIGHT, SCREEN_WIDTH},
    gba::Gba,
    save_file::SaveFile,
};

#[cfg(feature = "cpu-trace")]
use crate::bus::Bus;

/*
 * GBA master clock:
 *
 * 2^24 Hz = 16,777,216 cycles per second.
 */
const GBA_CLOCK_HZ: u64 = 16_777_216;
const SAVE_FLUSH_INTERVAL_CYCLES: u64 = 5 * GBA_CLOCK_HZ;

/*
 * One GBA frame:
 *
 * 228 scanlines × 1232 cycles.
 */
const CYCLES_PER_FRAME: u64 = 228 * 1232;

/*
 * Approximately 59.7275 frames per second.
 */
const FRAME_DURATION: Duration = Duration::from_nanos(
    (1_000_000_000u128 * CYCLES_PER_FRAME as u128 / GBA_CLOCK_HZ as u128) as u64,
);

const DEFAULT_SCALE: u32 = 3;

const WINDOW_WIDTH: u32 = SCREEN_WIDTH as u32 * DEFAULT_SCALE;

const WINDOW_HEIGHT: u32 = SCREEN_HEIGHT as u32 * DEFAULT_SCALE;

#[cfg(feature = "cpu-trace")]
const DEBUG_INTERVAL_CYCLES: u64 = 1_000_000;

const SPEED_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendAction {
    Continue,
    Quit,
}

fn process_events(
    event_pump: &mut sdl3::EventPump,
    gba: &mut Gba,
    paused: &mut bool,
) -> Result<FrontendAction> {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } => {
                return Ok(FrontendAction::Quit);
            }

            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => {
                return Ok(FrontendAction::Quit);
            }

            Event::KeyDown {
                keycode: Some(Keycode::Space),
                repeat: false,
                ..
            } => {
                *paused = !*paused;
            }

            Event::KeyDown {
                keycode: Some(keycode),
                repeat,
                ..
            } => {
                if !repeat && let Some(key) = map_keycode(keycode) {
                    gba.press_key(key);
                }
            }

            Event::KeyUp {
                keycode: Some(keycode),
                ..
            } => {
                if let Some(key) = map_keycode(keycode) {
                    gba.release_key(key);
                }
            }

            _ => {}
        }
    }

    Ok(FrontendAction::Continue)
}

pub fn run(gba: &mut Gba, save_file: &SaveFile) -> Result<()> {
    let sdl = sdl3::init().context("failed to initialize SDL3")?;

    let audio_spec = AudioSpec {
        freq: Some(AUDIO_OUTPUT_RATE as i32),
        channels: Some(2),
        format: Some(AudioFormat::S16LE),
    };

    let audio_stream = match sdl.audio() {
        Ok(audio_subsystem) => match audio_subsystem
            .open_playback_device(&audio_spec)
            .and_then(|device| device.open_device_stream(Some(&audio_spec)))
        {
            Ok(stream) => {
                stream
                    .resume()
                    .context("failed to start SDL3 audio playback")?;
                Some(stream)
            }
            Err(error) => {
                eprintln!("warning: audio output disabled: {error}");
                None
            }
        },
        Err(error) => {
            eprintln!("warning: SDL3 audio subsystem unavailable: {error}");
            None
        }
    };

    gba.set_audio_output_enabled(audio_stream.is_some());

    let video_subsystem = sdl
        .video()
        .context("failed to initialize SDL3 video subsystem")?;

    let window = video_subsystem
        .window("Running Man Advance", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .build()
        .context("failed to create SDL3 window")?;

    /*
     * SDL chooses an available accelerated renderer.
     */
    let mut canvas = window.into_canvas();

    canvas.set_draw_color(Color::RGB(0, 0, 0));

    canvas.clear();
    canvas.present();

    let texture_creator = canvas.texture_creator();

    /*
     * Our framebuffer stores each pixel as:
     *
     *     0xAARRGGBB
     *
     * Therefore ARGB8888 is the matching packed integer format.
     */
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormat::ARGB8888,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )
        .context("failed to create GBA framebuffer texture")?;

    /*
     * Preserve crisp pixels while scaling 240×160 to 720×480.
     */
    texture.set_scale_mode(ScaleMode::Nearest);

    let mut event_pump = sdl
        .event_pump()
        .context("failed to create SDL3 event pump")?;

    /*
     * Reusable host-side byte buffer.
     *
     * Texture::update accepts &[u8], while the emulator framebuffer is
     * &[u32]. We explicitly convert using native-endian bytes instead
     * of performing an unsafe slice cast.
     */
    let mut texture_pixels = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * size_of::<u32>()];
    let mut audio_samples = Vec::with_capacity(AUDIO_OUTPUT_RATE as usize / 30 * 2);

    let mut paused = false;
    let mut title_paused = false;
    let mut speed_sample_started = Instant::now();
    let mut speed_sample_cycles = gba.elapsed_cycles();
    let mut speed_sample_frames = gba.frame_number();
    let mut next_save_flush = gba
        .elapsed_cycles()
        .saturating_add(SAVE_FLUSH_INTERVAL_CYCLES);

    #[cfg(feature = "cpu-trace")]
    let mut next_debug_cycle = DEBUG_INTERVAL_CYCLES;

    /*
     * Render the initial framebuffer immediately.
     */
    upload_framebuffer(&mut texture_pixels, gba.framebuffer());

    texture.update(None, &texture_pixels, SCREEN_WIDTH * size_of::<u32>())?;

    canvas.copy(&texture, None, None)?;

    canvas.present();

    'running: loop {
        let frame_started = Instant::now();

        if process_events(&mut event_pump, gba, &mut paused)? == FrontendAction::Quit {
            break 'running;
        }

        if paused != title_paused {
            update_window_title(&mut canvas, paused, None)?;
            title_paused = paused;
            speed_sample_started = Instant::now();
            speed_sample_cycles = gba.elapsed_cycles();
            speed_sample_frames = gba.frame_number();

            if let Some(stream) = &audio_stream {
                if paused {
                    stream.pause().context("failed to pause SDL3 audio")?;
                } else {
                    stream.resume().context("failed to resume SDL3 audio")?;
                }
            }
        }

        if paused {
            thread::sleep(Duration::from_millis(10));

            continue;
        }

        /*
         * Run in small batches instead of blocking until an entire frame.
         *
         * This keeps the native window responsive even when emulation is
         * slow or the PPU does not generate frame-ready events.
         */
        const STEPS_PER_BATCH: usize = 1_024;
        const MAX_BATCHES_PER_HOST_FRAME: usize = 1_024;

        let mut frame_produced = false;

        for batch_index in 0..MAX_BATCHES_PER_HOST_FRAME {
            for _ in 0..STEPS_PER_BATCH {
                let cycles = gba.step();

                if cycles == 0 {
                    break;
                }

                #[cfg(feature = "cpu-trace")]
                if gba.elapsed_cycles() >= next_debug_cycle {
                    log_emulator_state(gba);

                    /*
                     * Dùng while thay vì cộng một lần để xử lý trường hợp một step
                     * lớn vượt qua nhiều mốc debug.
                     */
                    while next_debug_cycle <= gba.elapsed_cycles() {
                        next_debug_cycle = next_debug_cycle.saturating_add(DEBUG_INTERVAL_CYCLES);
                    }
                }

                if gba.take_frame_ready() {
                    frame_produced = true;
                    break;
                }
            }

            if frame_produced || gba.is_stopped() {
                break;
            }

            /*
             * Process native window events regularly while the emulator is
             * working toward the next frame.
             */
            if batch_index % 4 == 0 {
                if process_events(&mut event_pump, gba, &mut paused)? == FrontendAction::Quit {
                    break 'running;
                }

                if paused {
                    break;
                }
            }
        }

        if !frame_produced {
            /*
             * Emulator has not reached VBlank yet. Keep the window alive
             * and try again on the next frontend iteration.
             */
            thread::yield_now();
            continue;
        }

        upload_framebuffer(&mut texture_pixels, gba.framebuffer());

        texture.update(None, &texture_pixels, SCREEN_WIDTH * size_of::<u32>())?;

        canvas.set_draw_color(Color::RGB(0, 0, 0));

        canvas.clear();

        canvas.copy(&texture, None, None)?;

        canvas.present();

        if let Some(stream) = &audio_stream {
            audio_samples.clear();
            gba.drain_audio_samples(&mut audio_samples);

            if !audio_samples.is_empty() {
                stream
                    .put_data_i16(&audio_samples)
                    .context("failed to queue SDL3 audio samples")?;
            }
        }

        let sample_elapsed = speed_sample_started.elapsed();

        if sample_elapsed >= SPEED_SAMPLE_INTERVAL {
            let seconds = sample_elapsed.as_secs_f64();
            let elapsed_cycles = gba.elapsed_cycles().wrapping_sub(speed_sample_cycles);
            let elapsed_frames = gba.frame_number().wrapping_sub(speed_sample_frames);
            let realtime_percent = elapsed_cycles as f64 / seconds / GBA_CLOCK_HZ as f64 * 100.0;
            let frames_per_second = elapsed_frames as f64 / seconds;

            update_window_title(
                &mut canvas,
                false,
                Some((realtime_percent, frames_per_second)),
            )?;

            speed_sample_started = Instant::now();
            speed_sample_cycles = gba.elapsed_cycles();
            speed_sample_frames = gba.frame_number();
        }

        if gba.elapsed_cycles() >= next_save_flush {
            save_file.flush_if_dirty(gba)?;

            while next_save_flush <= gba.elapsed_cycles() {
                next_save_flush = next_save_flush.saturating_add(SAVE_FLUSH_INTERVAL_CYCLES);
            }
        }

        pace_frame(frame_started);
    }

    Ok(())
}

#[cfg(feature = "cpu-trace")]
fn log_emulator_state(gba: &Gba) {
    let dispcnt = gba.bus().read16(Bus::REG_DISPCNT);

    let vcount = gba.bus().read16(Bus::REG_VCOUNT);

    let mode = dispcnt & 0b111;

    let forced_blank = dispcnt & (1 << 7) != 0;

    let bg2_enabled = dispcnt & (1 << 10) != 0;

    println!(
        "cycles={} \
         pc=0x{:08X} \
         state={:?} \
         dispcnt=0x{dispcnt:04X} \
         mode={mode} \
         bg2={} \
         forced_blank={} \
         vcount={} \
         frame={} \
         halted={}",
        gba.elapsed_cycles(),
        gba.registers().pc(),
        gba.state(),
        bg2_enabled,
        forced_blank,
        vcount,
        gba.frame_number(),
        gba.cpu().is_halted(),
    );
}

fn map_keycode(keycode: Keycode) -> Option<Key> {
    match keycode {
        /*
         * Face buttons.
         */
        Keycode::Z => Some(Key::A),
        Keycode::X => Some(Key::B),

        /*
         * Shoulder buttons.
         */
        Keycode::A => Some(Key::L),
        Keycode::S => Some(Key::R),

        /*
         * Menu buttons.
         */
        Keycode::Backspace => Some(Key::Select),

        Keycode::Return => Some(Key::Start),

        /*
         * Directional pad.
         */
        Keycode::Right => Some(Key::Right),

        Keycode::Left => Some(Key::Left),

        Keycode::Up => Some(Key::Up),

        Keycode::Down => Some(Key::Down),

        _ => None,
    }
}

fn upload_framebuffer(destination: &mut [u8], framebuffer: &[u32]) {
    let expected_pixel_count = SCREEN_WIDTH * SCREEN_HEIGHT;

    assert_eq!(
        framebuffer.len(),
        expected_pixel_count,
        "GBA framebuffer has an invalid pixel count",
    );

    assert_eq!(
        destination.len(),
        expected_pixel_count * size_of::<u32>(),
        "SDL upload buffer has an invalid size",
    );

    for (destination_pixel, source_pixel) in destination
        .chunks_exact_mut(size_of::<u32>())
        .zip(framebuffer.iter().copied())
    {
        destination_pixel.copy_from_slice(&source_pixel.to_ne_bytes());
    }
}

fn pace_frame(frame_started: Instant) {
    let elapsed = frame_started.elapsed();

    if elapsed < FRAME_DURATION {
        thread::sleep(FRAME_DURATION - elapsed);
    }
}

fn update_window_title(
    canvas: &mut sdl3::render::WindowCanvas,
    paused: bool,
    performance: Option<(f64, f64)>,
) -> Result<()> {
    let title = if paused {
        "Running Man Advance [Paused]".to_owned()
    } else if let Some((realtime_percent, frames_per_second)) = performance {
        format!("Running Man Advance - {realtime_percent:.1}% - {frames_per_second:.1} FPS")
    } else {
        "Running Man Advance".to_owned()
    };

    canvas
        .window_mut()
        .set_title(&title)
        .context("failed to update SDL window title")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{map_keycode, upload_framebuffer};

    use crate::bus::{Key, SCREEN_HEIGHT, SCREEN_WIDTH};

    use sdl3::keyboard::Keycode;

    #[test]
    fn keyboard_mapping_matches_gba_buttons() {
        assert_eq!(map_keycode(Keycode::Z), Some(Key::A),);

        assert_eq!(map_keycode(Keycode::X), Some(Key::B),);

        assert_eq!(map_keycode(Keycode::Return,), Some(Key::Start),);

        assert_eq!(map_keycode(Keycode::Backspace,), Some(Key::Select),);

        assert_eq!(map_keycode(Keycode::Right,), Some(Key::Right),);
    }

    #[test]
    fn framebuffer_pixels_are_encoded_as_native_u32_bytes() {
        let framebuffer = vec![0xFFFF_0000u32; SCREEN_WIDTH * SCREEN_HEIGHT];

        let mut bytes = vec![0u8; framebuffer.len() * 4];

        upload_framebuffer(&mut bytes, &framebuffer);

        assert_eq!(&bytes[0..4], &0xFFFF_0000u32.to_ne_bytes(),);
    }
}
