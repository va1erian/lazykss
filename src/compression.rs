pub struct Decompressor<'a> {
    input: &'a [u8],
    pos: usize,
    output: Vec<u8>,
}

impl<'a> Decompressor<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: 0,
            output: Vec::with_capacity(0x10000), // Max 64KB
        }
    }

    pub fn decompress(mut self) -> Vec<u8> {
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > 20000 || self.pos >= self.input.len() {
                break;
            }

            let control = self.input[self.pos];
            self.pos += 1;

            if control == 0xFF {
                break;
            }

            let mode = control >> 5;
            let mut len = ((control & 0x1F) as usize) + 1;

            if mode == 7 {
                // Extended command
                if self.pos + 1 >= self.input.len() { break; }
                let sub_mode = (control & 0x1C) >> 2;
                len = (((control & 0x03) as usize) << 8) | (self.input[self.pos] as usize);
                len += 1;
                self.pos += 1;
                
                self.execute_mode(sub_mode, len);
            } else {
                self.execute_mode(mode, len);
            }
            
            // Safety: cap output size to 64KB
            if self.output.len() > 0x10000 { break; }
        }
        self.output
    }

    fn execute_mode(&mut self, mode: u8, len: usize) {
        match mode {
            0 => { // Raw data
                for _ in 0..len {
                    if self.pos < self.input.len() {
                        self.output.push(self.input[self.pos]);
                        self.pos += 1;
                    }
                }
            }
            1 => { // RLE 8
                if self.pos < self.input.len() {
                    let val = self.input[self.pos];
                    self.pos += 1;
                    for _ in 0..len {
                        self.output.push(val);
                    }
                }
            }
            2 => { // RLE 16
                if self.pos + 1 < self.input.len() {
                    let b1 = self.input[self.pos];
                    let b2 = self.input[self.pos + 1];
                    self.pos += 2;
                    for _ in 0..len {
                        self.output.push(b1);
                        self.output.push(b2);
                    }
                }
            }
            3 => { // RLE Ascending
                if self.pos < self.input.len() {
                    let mut val = self.input[self.pos];
                    self.pos += 1;
                    for _ in 0..len {
                        self.output.push(val);
                        val = val.wrapping_add(1);
                    }
                }
            }
            4 => { // LZ Forward
                if self.pos + 1 < self.input.len() {
                    let offset = u16::from_be_bytes([self.input[self.pos], self.input[self.pos + 1]]) as usize;
                    self.pos += 2;
                    for i in 0..len {
                        if offset + i < self.output.len() {
                            let val = self.output[offset + i];
                            self.output.push(val);
                        }
                    }
                }
            }
            5 => { // LZ Inverted / Bit-reversed
                if self.pos + 1 < self.input.len() {
                    let offset = u16::from_be_bytes([self.input[self.pos], self.input[self.pos + 1]]) as usize;
                    self.pos += 2;
                    for i in 0..len {
                        if offset + i < self.output.len() {
                            let val = self.output[offset + i];
                            self.output.push(val.reverse_bits());
                        }
                    }
                }
            }
            6 => { // LZ Reversed (reading backwards from offset)
                if self.pos + 1 < self.input.len() {
                    let offset = u16::from_be_bytes([self.input[self.pos], self.input[self.pos + 1]]) as usize;
                    self.pos += 2;
                    for i in 0..len {
                        if offset >= i && (offset - i) < self.output.len() {
                            let val = self.output[offset - i];
                            self.output.push(val);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompression() {
        // Command 0: Literal.
        let input1 = vec![0x02, 0xAA, 0xBB, 0xCC, 0xFF];
        let dec1 = Decompressor::new(&input1);
        assert_eq!(dec1.decompress(), vec![0xAA, 0xBB, 0xCC]);

        // Command 1: RLE 8.
        let input2 = vec![0x24, 0xDD, 0xFF];
        let dec2 = Decompressor::new(&input2);
        assert_eq!(dec2.decompress(), vec![0xDD; 5]);
    }
}
