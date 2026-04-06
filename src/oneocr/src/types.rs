use std::path::Path;

use serde::Serialize;

use crate::error::OcrError;

const MIN_DIM: u32 = 50;
const MAX_DIM: u32 = 10_000;

// ---------------------------------------------------------------------------
// OCR result types (public, serializable)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct OcrResult {
    pub text: String,
    pub text_angle: Option<f32>,
    pub lines: Vec<OcrLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrLine {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
    pub words: Vec<OcrWord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrWord {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub x3: f32,
    pub y3: f32,
    pub x4: f32,
    pub y4: f32,
}

impl BoundingBox {
    /// Leftmost x coordinate.
    pub fn left(&self) -> f32 {
        self.x1.min(self.x2).min(self.x3).min(self.x4)
    }

    /// Rightmost x coordinate.
    pub fn right(&self) -> f32 {
        self.x1.max(self.x2).max(self.x3).max(self.x4)
    }

    /// Topmost y coordinate.
    pub fn top(&self) -> f32 {
        self.y1.min(self.y2).min(self.y3).min(self.y4)
    }

    /// Bottommost y coordinate.
    pub fn bottom(&self) -> f32 {
        self.y1.max(self.y2).max(self.y3).max(self.y4)
    }

    pub fn width(&self) -> f32 {
        self.right() - self.left()
    }

    pub fn height(&self) -> f32 {
        self.bottom() - self.top()
    }
}

// ---------------------------------------------------------------------------
// Image wrapper (BGRA pixel data ready for the DLL)
// ---------------------------------------------------------------------------

pub struct OcrImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bgra_data: Vec<u8>,
}

impl OcrImage {
    /// Create from raw BGRA pixel data.
    pub fn from_bgra(width: u32, height: u32, bgra_data: Vec<u8>) -> Result<Self, OcrError> {
        validate_dimensions(width, height)?;
        validate_buffer(width, height, &bgra_data)?;
        Ok(Self {
            width,
            height,
            bgra_data,
        })
    }

    /// Create from raw RGBA pixel data. Channels are swapped to BGRA internally.
    pub fn from_rgba(width: u32, height: u32, mut data: Vec<u8>) -> Result<Self, OcrError> {
        validate_dimensions(width, height)?;
        validate_buffer(width, height, &data)?;
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // R <-> B
        }
        Ok(Self {
            width,
            height,
            bgra_data: data,
        })
    }

    /// Load any image file (PNG, JPEG, BMP, GIF, TIFF, WebP).
    pub fn open(path: &Path) -> Result<Self, OcrError> {
        let img = image::open(path)?;
        let rgba = img.to_rgba8();
        Self::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
    }

    /// Decode an image from in-memory bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OcrError> {
        let img = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        Self::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), OcrError> {
    if width < MIN_DIM || height < MIN_DIM || width > MAX_DIM || height > MAX_DIM {
        return Err(OcrError::ImageDimensions);
    }
    Ok(())
}

fn validate_buffer(width: u32, height: u32, data: &[u8]) -> Result<(), OcrError> {
    let expected = width as usize * height as usize * 4;
    if data.len() != expected {
        return Err(OcrError::BufferSize {
            expected,
            actual: data.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, x4: f32, y4: f32) -> BoundingBox {
        BoundingBox { x1, y1, x2, y2, x3, y3, x4, y4 }
    }

    // -- BoundingBox ----------------------------------------------------------

    #[test]
    fn bbox_axis_aligned() {
        // Standard rectangle: TL(10,20) TR(100,20) BR(100,50) BL(10,50)
        let b = bbox(10.0, 20.0, 100.0, 20.0, 100.0, 50.0, 10.0, 50.0);
        assert_eq!(b.left(), 10.0);
        assert_eq!(b.right(), 100.0);
        assert_eq!(b.top(), 20.0);
        assert_eq!(b.bottom(), 50.0);
        assert_eq!(b.width(), 90.0);
        assert_eq!(b.height(), 30.0);
    }

    #[test]
    fn bbox_rotated_clockwise() {
        // ~30 degree clockwise rotation: vertices shift so any could be extremal
        // TL(20,10) TR(90,30) BR(80,60) BL(10,40)
        let b = bbox(20.0, 10.0, 90.0, 30.0, 80.0, 60.0, 10.0, 40.0);
        assert_eq!(b.left(), 10.0);   // x4
        assert_eq!(b.right(), 90.0);  // x2
        assert_eq!(b.top(), 10.0);    // y1
        assert_eq!(b.bottom(), 60.0); // y3
    }

    #[test]
    fn bbox_rotated_counterclockwise() {
        // Rotated so bottom-left vertex has smallest x and top-right has largest y
        // TL(30,50) TR(100,20) BR(110,50) BL(40,80)
        let b = bbox(30.0, 50.0, 100.0, 20.0, 110.0, 50.0, 40.0, 80.0);
        assert_eq!(b.left(), 30.0);    // x1
        assert_eq!(b.right(), 110.0);  // x3
        assert_eq!(b.top(), 20.0);     // y2
        assert_eq!(b.bottom(), 80.0);  // y4
    }

    #[test]
    fn bbox_zero_size() {
        let b = bbox(5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0);
        assert_eq!(b.width(), 0.0);
        assert_eq!(b.height(), 0.0);
    }

    // -- Dimension validation -------------------------------------------------

    #[test]
    fn dimensions_valid_minimum() {
        assert!(validate_dimensions(50, 50).is_ok());
    }

    #[test]
    fn dimensions_valid_maximum() {
        assert!(validate_dimensions(10_000, 10_000).is_ok());
    }

    #[test]
    fn dimensions_too_small_width() {
        assert!(validate_dimensions(49, 50).is_err());
    }

    #[test]
    fn dimensions_too_small_height() {
        assert!(validate_dimensions(50, 49).is_err());
    }

    #[test]
    fn dimensions_too_large() {
        assert!(validate_dimensions(10_001, 100).is_err());
        assert!(validate_dimensions(100, 10_001).is_err());
    }

    // -- Buffer validation ----------------------------------------------------

    #[test]
    fn buffer_valid_size() {
        let data = vec![0u8; 50 * 50 * 4];
        assert!(validate_buffer(50, 50, &data).is_ok());
    }

    #[test]
    fn buffer_too_small() {
        let data = vec![0u8; 100];
        let err = validate_buffer(50, 50, &data).unwrap_err();
        match err {
            OcrError::BufferSize { expected, actual } => {
                assert_eq!(expected, 50 * 50 * 4);
                assert_eq!(actual, 100);
            }
            _ => panic!("expected BufferSize error"),
        }
    }

    #[test]
    fn buffer_too_large() {
        let data = vec![0u8; 50 * 50 * 4 + 1];
        assert!(validate_buffer(50, 50, &data).is_err());
    }

    // -- OcrImage constructors ------------------------------------------------

    #[test]
    fn from_bgra_valid() {
        let data = vec![0u8; 50 * 50 * 4];
        let img = OcrImage::from_bgra(50, 50, data).unwrap();
        assert_eq!(img.width(), 50);
        assert_eq!(img.height(), 50);
    }

    #[test]
    fn from_bgra_rejects_bad_buffer() {
        let data = vec![0u8; 100];
        assert!(OcrImage::from_bgra(50, 50, data).is_err());
    }

    #[test]
    fn from_bgra_rejects_bad_dimensions() {
        let data = vec![0u8; 10 * 10 * 4];
        assert!(OcrImage::from_bgra(10, 10, data).is_err());
    }

    #[test]
    fn from_rgba_swaps_channels() {
        // Single pixel: R=0xFF G=0x00 B=0xAA A=0xBB
        let mut data = vec![0u8; 50 * 50 * 4];
        data[0] = 0xFF; // R
        data[1] = 0x00; // G
        data[2] = 0xAA; // B
        data[3] = 0xBB; // A
        let img = OcrImage::from_rgba(50, 50, data).unwrap();
        // After swap: B=0xFF G=0x00 R=0xAA A=0xBB
        assert_eq!(img.bgra_data[0], 0xAA); // B (was R position, now has B value)
        assert_eq!(img.bgra_data[1], 0x00); // G unchanged
        assert_eq!(img.bgra_data[2], 0xFF); // R (was B position, now has R value)
        assert_eq!(img.bgra_data[3], 0xBB); // A unchanged
    }

    #[test]
    fn from_rgba_rejects_bad_buffer() {
        let data = vec![0u8; 100];
        assert!(OcrImage::from_rgba(50, 50, data).is_err());
    }

    #[test]
    fn open_nonexistent_file() {
        assert!(OcrImage::open(Path::new("nonexistent_image_12345.png")).is_err());
    }

    #[test]
    fn from_bytes_invalid_data() {
        assert!(OcrImage::from_bytes(&[0, 1, 2, 3]).is_err());
    }
}
