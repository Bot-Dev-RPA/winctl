use thiserror::Error;

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("failed to load library: {0}")]
    LibraryLoad(#[from] libloading::Error),

    #[error("missing DLL export: {name}")]
    MissingSymbol { name: &'static str },

    #[error("engine not found -- {hint}")]
    EngineNotFound { hint: String },

    #[error("{operation} failed (code {code})")]
    DllCall { operation: &'static str, code: i64 },

    #[error("{0}")]
    Image(#[from] image::ImageError),

    #[error("image dimensions out of range (need 50..10000 per axis)")]
    ImageDimensions,

    #[error("buffer size mismatch: expected {expected} bytes, got {actual}")]
    BufferSize { expected: usize, actual: usize },

    #[error("clipboard: {0}")]
    Clipboard(String),

    #[error("auto-setup failed: {hint}")]
    SetupFailed { hint: String },

    #[error("null byte in path")]
    NulInPath(#[from] std::ffi::NulError),
}
