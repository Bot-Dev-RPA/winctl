#[cfg(not(target_os = "windows"))]
compile_error!("oneocr requires Windows");

mod engine;
mod error;
mod ffi;
pub mod format;
mod types;

pub use engine::{find_snipping_tool_path, resolve_engine_dir, setup_engine, OcrEngine};
pub use error::OcrError;
pub use format::{detect_tables, to_spaced_text, to_table_text, Table};
pub use types::{BoundingBox, OcrImage, OcrLine, OcrResult, OcrWord};
