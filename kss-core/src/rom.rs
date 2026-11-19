//! The ROM image: loading, identification, raw access and the checksum.
//!
//! All offsets are PC file offsets. The SA-1 maps the 4 MiB image linearly
//! at banks `C0-FF`, so `pc = snes - 0xC00000` for every long pointer the
//! data tables hold.

use std::path::{Path, PathBuf};

/// File name the tools look for when no path is given.
pub const DEFAULT_ROM_NAME: &str = "Kirby Super Star (USA).sfc";
/// Size of the headerless US image.
pub const ROM_SIZE: usize = 0x40_0000;
const TITLE_OFFSET: usize = 0x7FC0;
const TITLE: &[u8] = b"KIRBY SUPER DELUXE";
const CHECKSUM_COMPLEMENT: usize = 0x7FDC;
const CHECKSUM: usize = 0x7FDE;

#[derive(Debug)]
pub enum RomError {
    Io(std::io::Error),
    /// Not the headerless 4 MiB US image.
    NotKirbySuperStar(String),
}

impl std::fmt::Display for RomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::NotKirbySuperStar(why) => write!(f, "not a Kirby Super Star (USA) ROM: {why}"),
        }
    }
}

impl std::error::Error for RomError {}

impl From<std::io::Error> for RomError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Clone)]
pub struct Rom {
    data: Vec<u8>,
}

impl Rom {
    /// Wraps an image, dropping a 512-byte copier header if there is one.
    pub fn new(mut data: Vec<u8>) -> Result<Self, RomError> {
        if data.len() == ROM_SIZE + 0x200 {
            data.drain(..0x200);
        }
        if data.len() != ROM_SIZE {
            return Err(RomError::NotKirbySuperStar(format!(
                "expected {ROM_SIZE} bytes, found {}",
                data.len()
            )));
        }
        if &data[TITLE_OFFSET..TITLE_OFFSET + TITLE.len()] != TITLE {
            return Err(RomError::NotKirbySuperStar(
                "internal title does not match".into(),
            ));
        }
        Ok(Self { data })
    }

    pub fn load(path: &Path) -> Result<Self, RomError> {
        Self::new(std::fs::read(path)?)
    }

    /// Writes the image with a fresh checksum. The data goes to a temporary
    /// file beside `path` that then replaces it, so a write that fails
    /// part-way (a full disk) leaves an existing file as it was.
    pub fn save(&mut self, path: &Path) -> Result<(), RomError> {
        self.fix_checksum();
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".writing");
        let temporary = path.with_file_name(name);
        let written =
            std::fs::write(&temporary, &self.data).and_then(|()| std::fs::rename(&temporary, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        Ok(written?)
    }

    /// Looks for the ROM: `KSS_ROM`, then [`DEFAULT_ROM_NAME`] in the current
    /// directory and each of its ancestors.
    pub fn locate() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("KSS_ROM")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
        {
            return Some(path);
        }
        let cwd = std::env::current_dir().ok()?;
        cwd.ancestors()
            .map(|dir| dir.join(DEFAULT_ROM_NAME))
            .find(|p| p.is_file())
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn u8(&self, offset: usize) -> u8 {
        self.data[offset]
    }

    pub fn u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.data[offset], self.data[offset + 1]])
    }

    /// Reads a 3-byte SNES long pointer and converts it to a file offset.
    /// `None` when it does not point into the `C0-FF` image.
    pub fn long_pointer(&self, offset: usize) -> Option<usize> {
        let snes = self.data[offset] as usize
            | (self.data[offset + 1] as usize) << 8
            | (self.data[offset + 2] as usize) << 16;
        snes.checked_sub(0xC0_0000)
            .filter(|&pc| pc < self.data.len())
    }

    pub fn slice(&self, offset: usize, len: usize) -> &[u8] {
        &self.data[offset..offset + len]
    }

    pub fn write(&mut self, offset: usize, bytes: &[u8]) {
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    pub fn write_long_pointer(&mut self, offset: usize, pc: usize) {
        let snes = pc + 0xC0_0000;
        self.write(offset, &[snes as u8, (snes >> 8) as u8, (snes >> 16) as u8]);
    }

    /// Recomputes the internal checksum pair (sum of all bytes mod 0x10000,
    /// taken with the checksum fields at their neutral `FFFF`/`0000` values).
    pub fn fix_checksum(&mut self) {
        self.write(CHECKSUM_COMPLEMENT, &[0xFF, 0xFF, 0x00, 0x00]);
        let sum = self
            .data
            .iter()
            .fold(0u16, |acc, &b| acc.wrapping_add(b as u16));
        self.write(CHECKSUM, &sum.to_le_bytes());
        self.write(CHECKSUM_COMPLEMENT, &(!sum).to_le_bytes());
    }

    pub fn checksum(&self) -> u16 {
        self.u16(CHECKSUM)
    }
}

/// Converts an SNES `C0-FF` address to a file offset.
pub fn snes_to_pc(snes: usize) -> Option<usize> {
    snes.checked_sub(0xC0_0000).filter(|&pc| pc < ROM_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_conversion() {
        assert_eq!(snes_to_pc(0xFF_0000), Some(0x3F_0000));
        assert_eq!(snes_to_pc(0xF0_20EF), Some(0x30_20EF));
        assert_eq!(snes_to_pc(0x00_8000), None);
    }

    #[test]
    fn rejects_other_files() {
        assert!(Rom::new(vec![0; 1024]).is_err());
        assert!(Rom::new(vec![0; ROM_SIZE]).is_err());
    }

    #[test]
    fn checksum_of_the_original_is_unchanged() {
        let Some(path) = Rom::locate() else { return };
        let mut rom = Rom::load(&path).unwrap();
        let before = rom.checksum();
        rom.fix_checksum();
        assert_eq!(rom.checksum(), before);
        assert_eq!(before, 0xC6AA);
    }
}
