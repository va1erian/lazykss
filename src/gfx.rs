#[allow(dead_code)]
pub struct Palette {
    pub colors: Vec<egui::Color32>,
}

impl Palette {
    #[allow(dead_code)]
    pub fn from_snes_bytes(data: &[u8]) -> Self {
        let mut colors = Vec::new();
        for i in (0..data.len()).step_by(2) {
            let b1 = data[i];
            let b2 = data[i + 1];
            let val = u16::from_le_bytes([b1, b2]);
            
            // SNES BGR555: 0BBBBBGGGGGRRRRR
            let r = ((val & 0x1F) << 3) as u8;
            let g = (((val >> 5) & 0x1F) << 3) as u8;
            let b = (((val >> 10) & 0x1F) << 3) as u8;
            
            colors.push(egui::Color32::from_rgb(r, g, b));
        }
        Self { colors }
    }
}
