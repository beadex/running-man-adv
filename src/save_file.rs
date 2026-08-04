use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use crate::gba::Gba;

static NEXT_WORK_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct SaveFile {
    path: PathBuf,
}

impl SaveFile {
    pub fn for_rom(rom_path: &Path) -> Self {
        Self::new(rom_path.with_extension("sav"))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self, gba: &mut Gba) -> Result<bool> {
        let data = match fs::read(&self.path) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read save file {}", self.path.display()));
            }
        };

        gba.load_cartridge_save(&data).with_context(|| {
            format!(
                "save file {} does not match the detected {} cartridge save",
                self.path.display(),
                gba.cartridge_save_type().name()
            )
        })?;

        Ok(true)
    }

    pub fn flush_if_dirty(&self, gba: &mut Gba) -> Result<bool> {
        if !gba.cartridge_save_dirty() {
            return Ok(false);
        }

        atomic_replace(&self.path, gba.cartridge_save_data())
            .with_context(|| format!("failed to write save file {}", self.path.display()))?;

        gba.mark_cartridge_save_clean();

        Ok(true)
    }
}

fn atomic_replace(path: &Path, data: &[u8]) -> io::Result<()> {
    let temporary_path = unique_sibling_path(path, "tmp");
    let mut temporary = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)?;

    let write_result = (|| {
        temporary.write_all(data)?;
        temporary.sync_all()?;
        drop(temporary);

        install_temporary_file(&temporary_path, path)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }

    write_result
}

#[cfg(not(windows))]
fn install_temporary_file(temporary_path: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary_path, destination)
}

#[cfg(windows)]
fn install_temporary_file(temporary_path: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        return fs::rename(temporary_path, destination);
    }

    /*
     * std::fs::rename does not replace an existing file on Windows. Keep the
     * previous valid save recoverable until the new file is installed.
     */
    let backup_path = unique_sibling_path(destination, "bak");
    fs::rename(destination, &backup_path)?;

    match fs::rename(temporary_path, destination) {
        Ok(()) => {
            let _ = fs::remove_file(backup_path);
            Ok(())
        }
        Err(install_error) => match fs::rename(&backup_path, destination) {
            Ok(()) => Err(install_error),
            Err(restore_error) => Err(io::Error::other(format!(
                "failed to install new save ({install_error}) and restore previous save ({restore_error}); backup remains at {}",
                backup_path.display()
            ))),
        },
    }
}

fn unique_sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let id = NEXT_WORK_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let process = std::process::id();
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("save"))
        .to_string_lossy();

    path.with_file_name(format!(".{file_name}.{process}.{id}.{suffix}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::SaveFile;
    use crate::gba::Gba;

    const SAVE_BASE: u32 = 0x0E00_0000;

    fn temporary_save_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "running-man-adv-{name}-{}-{}.sav",
            std::process::id(),
            super::NEXT_WORK_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    fn program_flash(gba: &mut Gba, offset: u32, value: u8) {
        let bus = gba.bus_mut();
        bus.write8(SAVE_BASE + 0x5555, 0xAA);
        bus.write8(SAVE_BASE + 0x2AAA, 0x55);
        bus.write8(SAVE_BASE + 0x5555, 0xA0);
        bus.write8(SAVE_BASE + offset, value);
    }

    #[test]
    fn flash_save_round_trips_through_disk() {
        let path = temporary_save_path("round-trip");
        let save_file = SaveFile::new(path.clone());
        let mut first = Gba::new();
        first.load_rom(b"FLASH1M_V103").unwrap();
        program_flash(&mut first, 0x1234, 0x5A);

        assert!(save_file.flush_if_dirty(&mut first).unwrap());
        assert!(!first.cartridge_save_dirty());
        assert_eq!(fs::metadata(&path).unwrap().len(), 128 * 1024);

        /* Replacing an existing save exercises the Windows backup swap. */
        program_flash(&mut first, 0x2345, 0xA6);
        assert!(save_file.flush_if_dirty(&mut first).unwrap());

        let mut second = Gba::new();
        second.load_rom(b"FLASH1M_V103").unwrap();
        assert!(save_file.load(&mut second).unwrap());
        assert_eq!(second.bus().read8(SAVE_BASE + 0x1234), 0x5A);
        assert_eq!(second.bus().read8(SAVE_BASE + 0x2345), 0xA6);
        assert!(!second.cartridge_save_dirty());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_existing_file_is_rejected_without_becoming_dirty() {
        let path = temporary_save_path("invalid-size");
        fs::write(&path, [0xA5; 17]).unwrap();

        let save_file = SaveFile::new(path.clone());
        let mut gba = Gba::new();
        gba.load_rom(b"FLASH1M_V103").unwrap();

        assert!(save_file.load(&mut gba).is_err());
        assert!(!gba.cartridge_save_dirty());
        assert_eq!(fs::read(&path).unwrap(), [0xA5; 17]);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn clean_save_does_not_create_a_file() {
        let path = temporary_save_path("clean");
        let save_file = SaveFile::new(path.clone());
        let mut gba = Gba::new();
        gba.load_rom(b"FLASH1M_V103").unwrap();

        assert!(!save_file.flush_if_dirty(&mut gba).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn sram_save_round_trips_through_disk() {
        let path = temporary_save_path("sram-round-trip");
        let save_file = SaveFile::new(path.clone());
        let mut first = Gba::new();
        first.load_rom(b"SRAM_V110").unwrap();
        first.bus_mut().write8(SAVE_BASE + 0x4321, 0x7C);

        assert!(save_file.flush_if_dirty(&mut first).unwrap());
        assert_eq!(fs::metadata(&path).unwrap().len(), 64 * 1024);

        let mut second = Gba::new();
        second.load_rom(b"SRAM_V110").unwrap();
        assert!(save_file.load(&mut second).unwrap());
        assert_eq!(second.bus().read8(SAVE_BASE + 0x4321), 0x7C);

        fs::remove_file(path).unwrap();
    }
}
