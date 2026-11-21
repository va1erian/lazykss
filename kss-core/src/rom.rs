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
        if !data
            .get(TITLE_OFFSET..)
            .is_some_and(|rest| rest.starts_with(TITLE))
        {
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

    /// The byte at `offset`, 0 past the end of the image. A convenience for
    /// offsets known to be inside it: a parser of offsets read from the ROM
    /// checks them first, as `room` does with its own bounds-checked reads.
    pub fn u8(&self, offset: usize) -> u8 {
        self.data.get(offset).copied().unwrap_or(0)
    }

    /// The little-endian word at `offset`, 0 when it is not entirely inside
    /// the image.
    pub fn u16(&self, offset: usize) -> u16 {
        self.bytes::<2>(offset).map_or(0, u16::from_le_bytes)
    }

    /// `N` bytes at `offset`, when they are all inside the image.
    fn bytes<const N: usize>(&self, offset: usize) -> Option<[u8; N]> {
        self.data.get(offset..)?.first_chunk::<N>().copied()
    }

    /// Reads a 3-byte SNES long pointer and converts it to a file offset.
    /// `None` when it does not point into the `C0-FF` image, or when the
    /// pointer itself is not inside the image.
    pub fn long_pointer(&self, offset: usize) -> Option<usize> {
        let [low, high, bank] = self.bytes::<3>(offset)?;
        let snes = usize::from(low) | usize::from(high) << 8 | usize::from(bank) << 16;
        snes.checked_sub(0xC0_0000)
            .filter(|&pc| pc < self.data.len())
    }

    /// `len` bytes at `offset`; empty when the range leaves the image.
    pub fn slice(&self, offset: usize, len: usize) -> &[u8] {
        self.data
            .get(offset..)
            .and_then(|rest| rest.get(..len))
            .unwrap_or_default()
    }

    /// Overwrites bytes at `offset`. Returns false, with nothing written,
    /// when the bytes would leave the image.
    #[must_use]
    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> bool {
        let Some(target) = self
            .data
            .get_mut(offset..)
            .and_then(|rest| rest.get_mut(..bytes.len()))
        else {
            return false;
        };
        target.copy_from_slice(bytes);
        true
    }

    /// Writes a 3-byte SNES long pointer to file offset `pc`. Returns
    /// false, with nothing written, when `pc` is outside the image (no
    /// pointer can reach it) or the pointer itself would leave it.
    #[must_use]
    pub fn write_long_pointer(&mut self, offset: usize, pc: usize) -> bool {
        let Some(snes) = pc_to_snes(pc) else {
            return false;
        };
        let [low, high, bank, ..] = snes.to_le_bytes();
        self.write(offset, &[low, high, bank])
    }

    /// Recomputes the internal checksum pair (sum of all bytes mod 0x10000,
    /// taken with the checksum fields at their neutral `FFFF`/`0000` values).
    pub fn fix_checksum(&mut self) {
        // The header fields are inside every image `Rom::new` accepts, so
        // these writes cannot be refused.
        let _ = self.write(CHECKSUM_COMPLEMENT, &[0xFF, 0xFF, 0x00, 0x00]);
        let sum = self
            .data
            .iter()
            .fold(0u16, |acc, &b| acc.wrapping_add(b as u16));
        let _ = self.write(CHECKSUM, &sum.to_le_bytes());
        let _ = self.write(CHECKSUM_COMPLEMENT, &(!sum).to_le_bytes());
    }

    pub fn checksum(&self) -> u16 {
        self.u16(CHECKSUM)
    }
}

/// Converts a file offset to its SNES `C0-FF` address.
fn pc_to_snes(pc: usize) -> Option<usize> {
    (pc < ROM_SIZE).then_some(pc | 0xC0_0000)
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
        assert_eq!(pc_to_snes(0x3F_0000), Some(0xFF_0000));
        assert_eq!(pc_to_snes(ROM_SIZE), None);
    }

    #[test]
    fn reads_and_writes_past_the_end_do_not_panic() {
        let mut data = vec![0; ROM_SIZE];
        data[TITLE_OFFSET..TITLE_OFFSET + TITLE.len()].copy_from_slice(TITLE);
        let mut rom = Rom::new(data).unwrap();
        let end = ROM_SIZE - 1;
        assert!(rom.write(end, &[0xAB]));
        assert_eq!(rom.u8(end), 0xAB);
        assert_eq!(rom.u16(end), 0);
        assert_eq!(rom.u8(usize::MAX), 0);
        assert_eq!(rom.long_pointer(end - 1), None);
        assert!(rom.slice(end, 2).is_empty());
        assert!(rom.slice(usize::MAX, 2).is_empty());
        assert!(!rom.write(end, &[1, 2]));
        assert!(!rom.write(usize::MAX, &[1]));
        assert!(!rom.write_long_pointer(0, ROM_SIZE));
        assert!(!rom.write_long_pointer(end, 0));
        assert_eq!(rom.u8(end), 0xAB);
        assert_eq!(rom.slice(0, 3), [0, 0, 0]);
        assert!(rom.write_long_pointer(0, 0x12_3456));
        assert_eq!(rom.long_pointer(0), Some(0x12_3456));
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
