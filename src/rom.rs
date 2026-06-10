pub struct Rom {
    data: Vec<u8>,
}

impl Rom {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Converts a SNES LoROM address to a physical file offset.
    pub fn snes_to_offset(bank: u8, addr: u16) -> usize {
        let bank = bank as usize;
        let addr = addr as usize;
        let snes_addr = (bank << 16) | addr;
        if snes_addr >= 0xC00000 {
            snes_addr - 0xC00000
        } else {
            ((bank & 0x7F) * 0x8000) | (addr & 0x7FFF)
        }
    }

    #[allow(dead_code)]
    pub fn read_u8(&self, bank: u8, addr: u16) -> u8 {
        let offset = Self::snes_to_offset(bank, addr);
        self.read_u8_at_pc(offset)
    }

    pub fn read_u16(&self, bank: u8, addr: u16) -> u16 {
        let offset = Self::snes_to_offset(bank, addr);
        self.read_u16_at_pc(offset)
    }

    #[allow(dead_code)]
    pub fn read_slice(&self, bank: u8, addr: u16, length: usize) -> &[u8] {
        let offset = Self::snes_to_offset(bank, addr);
        self.read_slice_at_pc(offset, length)
    }

    pub fn read_u8_at_pc(&self, offset: usize) -> u8 {
        self.data[offset]
    }

    pub fn read_u16_at_pc(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.data[offset], self.data[offset + 1]])
    }

    pub fn read_slice_at_pc(&self, offset: usize, length: usize) -> &[u8] {
        &self.data[offset..offset + length]
    }
    
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snes_to_offset() {
        assert_eq!(Rom::snes_to_offset(0xFF, 0x0000), 0x3F0000);
        assert_eq!(Rom::snes_to_offset(0xF0, 0x20EF), 0x3020EF);
        assert_eq!(Rom::snes_to_offset(0xC0, 0x0000), 0x000000);
        assert_eq!(Rom::snes_to_offset(0x00, 0x8000), 0x000000);
    }
}
