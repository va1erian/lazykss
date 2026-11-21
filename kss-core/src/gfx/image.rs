//! A plain RGBA8 image, the output of every renderer.

/// An RGBA8 image, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Bytes of a `width` x `height` image, `None` when that overflows.
fn byte_len(width: usize, height: usize) -> Option<usize> {
    width.checked_mul(height)?.checked_mul(4)
}

impl Image {
    /// A transparent image. A size whose byte count overflows `usize` gives
    /// an empty 0x0 image.
    pub fn new(width: usize, height: usize) -> Self {
        match byte_len(width, height) {
            Some(len) => Self {
                width,
                height,
                rgba: vec![0; len],
            },
            None => Self::new(0, 0),
        }
    }

    /// An image of one colour, sized as [`Image::new`].
    pub fn filled(width: usize, height: usize, color: [u8; 4]) -> Self {
        match byte_len(width, height) {
            Some(len) => Self {
                width,
                height,
                rgba: color.into_iter().cycle().take(len).collect(),
            },
            None => Self::new(0, 0),
        }
    }

    /// Where pixel `(x, y)` starts in `rgba`, when it is inside the image.
    fn offset(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        y.checked_mul(self.width)?.checked_add(x)?.checked_mul(4)
    }

    fn pixel_bytes(&mut self, x: usize, y: usize) -> Option<&mut [u8]> {
        let at = self.offset(x, y)?;
        self.rgba.get_mut(at..)?.get_mut(..4)
    }

    /// The colour at `(x, y)`; transparent black outside the image.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        self.offset(x, y)
            .and_then(|at| self.rgba.get(at..)?.first_chunk::<4>())
            .copied()
            .unwrap_or_default()
    }

    /// Sets the colour at `(x, y)`. Pixels outside the image are ignored,
    /// so callers can draw shapes that hang over an edge.
    pub fn put(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if let Some(bytes) = self.pixel_bytes(x, y) {
            bytes.copy_from_slice(&color);
        }
    }
}
