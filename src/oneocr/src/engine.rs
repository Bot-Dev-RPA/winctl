use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};

use crate::error::OcrError;
use crate::ffi::{FfiBoundingBox, FfiImage, OcrLibrary};
use crate::types::{BoundingBox, OcrImage, OcrLine, OcrResult, OcrWord};

const MODEL_KEY: &[u8] = b"kj)TGtrK>f]b[Piow.gU+nC@s\"\"\"\"\"\"4";
const MODEL_FILE: &str = "oneocr.onemodel";
const ENGINE_FILES: &[&str] = &["oneocr.dll", MODEL_FILE, "onnxruntime.dll"];

// ---------------------------------------------------------------------------
// Engine directory resolution
// ---------------------------------------------------------------------------

fn engine_dir_ready(dir: &Path) -> bool {
    dir.join("oneocr.dll").exists() && dir.join(MODEL_FILE).exists()
}

/// Locate the directory containing `oneocr.dll` and `oneocr.onemodel`.
///
/// Search order:
///  1. `explicit` path (from CLI flag or direct call)
///  2. `ONEOCR_ENGINE_DIR` environment variable
///  3. `<executable>/engine/`
///  4. Auto-detect Snipping Tool install and copy engine files
pub fn resolve_engine_dir(explicit: Option<&Path>) -> Result<PathBuf, OcrError> {
    let candidates: Vec<PathBuf> = [
        explicit.map(Path::to_path_buf),
        std::env::var("ONEOCR_ENGINE_DIR").ok().map(PathBuf::from),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("engine"))),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in &candidates {
        if engine_dir_ready(dir) {
            return dir
                .canonicalize()
                .map_err(|_| OcrError::EngineNotFound {
                    hint: format!("cannot canonicalize {}", dir.display()),
                });
        }
    }

    // No engine found -- try auto-setup from Snipping Tool
    let target = candidates
        .last()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("engine"));

    setup_engine(&target, false)?;

    target
        .canonicalize()
        .map_err(|_| OcrError::EngineNotFound {
            hint: format!("auto-setup completed but cannot canonicalize {}", target.display()),
        })
}

/// Find the Snipping Tool's SnippingTool directory containing oneocr.dll.
///
/// Uses `Get-AppxPackage` to locate the install path since the WindowsApps
/// directory cannot be listed by non-admin processes.
pub fn find_snipping_tool_path() -> Option<PathBuf> {
    // Try Get-AppxPackage to find the install location
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-AppxPackage *ScreenSketch* | Select-Object -ExpandProperty InstallLocation",
        ])
        .output()
        .ok()?;

    let install_loc = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    if !install_loc.is_empty() {
        let snip_dir = PathBuf::from(&install_loc).join("SnippingTool");
        if snip_dir.join("oneocr.dll").exists() {
            return Some(snip_dir);
        }
    }

    None
}

/// Copy engine files from Snipping Tool to the target directory.
///
/// When `force` is true, all files are re-copied even if they already exist.
/// When false, existing files are skipped (used for automatic first-run setup).
pub fn setup_engine(target: &Path, force: bool) -> Result<(), OcrError> {
    let source = find_snipping_tool_path().ok_or_else(|| OcrError::SetupFailed {
        hint: "Snipping Tool with OCR not found. Is this Windows 11 with Snipping Tool 11.2409+?"
            .into(),
    })?;

    std::fs::create_dir_all(target).map_err(|e| OcrError::SetupFailed {
        hint: format!("cannot create {}: {}", target.display(), e),
    })?;

    eprintln!("Copying OCR engine from {}", source.display());

    for &file in ENGINE_FILES {
        let dst = target.join(file);
        if !force && dst.exists() {
            continue;
        }
        eprintln!("  copying {}...", file);
        std::fs::copy(source.join(file), &dst).map_err(|e| OcrError::SetupFailed {
            hint: format!("failed to copy {}: {}", file, e),
        })?;
    }

    if !engine_dir_ready(target) {
        return Err(OcrError::SetupFailed {
            hint: "files copied but engine directory validation failed".into(),
        });
    }

    eprintln!("Engine setup complete: {}", target.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// OcrEngine -- safe, RAII wrapper around the DLL handles
// ---------------------------------------------------------------------------

pub struct OcrEngine {
    lib: OcrLibrary,
    init_options: i64,
    pipeline: i64,
    process_options: i64,
}

impl OcrEngine {
    /// Load the engine from the given directory.
    ///
    /// The directory must contain `oneocr.dll`, `oneocr.onemodel`, and
    /// `onnxruntime.dll`.
    pub fn new(engine_dir: &Path) -> Result<Self, OcrError> {
        let lib = OcrLibrary::load(engine_dir)?;

        // Each step cleans up previously-created handles on failure.
        let init_options = create_init_options(&lib)?;

        let pipeline = create_pipeline(&lib, engine_dir, init_options).inspect_err(|_| unsafe {
            (lib.release_init_options)(init_options);
        })?;

        let process_options = create_process_options(&lib).inspect_err(|_| unsafe {
            (lib.release_pipeline)(pipeline);
            (lib.release_init_options)(init_options);
        })?;

        Ok(Self {
            lib,
            init_options,
            pipeline,
            process_options,
        })
    }

    /// Run OCR on an image and return structured results.
    pub fn recognize(&self, image: &OcrImage) -> Result<OcrResult, OcrError> {
        let ffi_img = FfiImage {
            image_type: 3, // BGRA / CV_8UC4
            width: image.width as i32,
            height: image.height as i32,
            _reserved: 0,
            step_size: (image.width * 4) as i64,
            data_ptr: image.bgra_data.as_ptr(),
        };

        let mut result_handle: i64 = 0;
        check(
            unsafe {
                (self.lib.run_pipeline)(
                    self.pipeline,
                    &ffi_img,
                    self.process_options,
                    &mut result_handle,
                )
            },
            "RunOcrPipeline",
        )?;

        let result = self.parse_result(result_handle);
        unsafe { (self.lib.release_result)(result_handle) };
        result
    }

    // -- result parsing -----------------------------------------------------

    fn parse_result(&self, handle: i64) -> Result<OcrResult, OcrError> {
        let mut count: i64 = 0;
        check(
            unsafe { (self.lib.get_line_count)(handle, &mut count) },
            "GetOcrLineCount",
        )?;

        let mut lines = Vec::with_capacity(count as usize);
        for i in 0..count {
            lines.push(self.parse_line(handle, i)?);
        }

        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let text_angle = {
            let mut angle: f32 = 0.0;
            if unsafe { (self.lib.get_angle)(handle, &mut angle) } == 0 {
                Some(angle)
            } else {
                None
            }
        };

        Ok(OcrResult {
            text,
            text_angle,
            lines,
        })
    }

    fn parse_line(&self, result: i64, index: i64) -> Result<OcrLine, OcrError> {
        let mut handle: i64 = 0;
        check(
            unsafe { (self.lib.get_line)(result, index, &mut handle) },
            "GetOcrLine",
        )?;

        let text = read_text(|p| unsafe { (self.lib.get_line_content)(handle, p) });
        let bounding_box = read_bbox(|p| unsafe { (self.lib.get_line_bbox)(handle, p) });

        let mut word_count: i64 = 0;
        let words = if unsafe { (self.lib.get_word_count)(handle, &mut word_count) } == 0 {
            (0..word_count)
                .filter_map(|i| self.parse_word(handle, i))
                .collect()
        } else {
            Vec::new()
        };

        Ok(OcrLine {
            text,
            bounding_box,
            words,
        })
    }

    fn parse_word(&self, line: i64, index: i64) -> Option<OcrWord> {
        let mut handle: i64 = 0;
        if unsafe { (self.lib.get_word)(line, index, &mut handle) } != 0 {
            return None;
        }

        let text = read_text(|p| unsafe { (self.lib.get_word_content)(handle, p) });
        let bounding_box = read_bbox(|p| unsafe { (self.lib.get_word_bbox)(handle, p) });

        let mut conf: f32 = 0.0;
        if unsafe { (self.lib.get_word_confidence)(handle, &mut conf) } != 0 {
            conf = 0.0;
        }

        Some(OcrWord {
            text,
            bounding_box,
            confidence: conf,
        })
    }
}

impl Drop for OcrEngine {
    fn drop(&mut self) {
        unsafe {
            (self.lib.release_process_options)(self.process_options);
            (self.lib.release_pipeline)(self.pipeline);
            (self.lib.release_init_options)(self.init_options);
        }
    }
}

// ---------------------------------------------------------------------------
// Initialization helpers
// ---------------------------------------------------------------------------

fn create_init_options(lib: &OcrLibrary) -> Result<i64, OcrError> {
    let mut handle: i64 = 0;
    check(
        unsafe { (lib.create_init_options)(&mut handle) },
        "CreateOcrInitOptions",
    )?;
    check(
        unsafe { (lib.set_delay_load)(handle, 0) },
        "SetUseModelDelayLoad",
    )?;
    Ok(handle)
}

fn create_pipeline(lib: &OcrLibrary, engine_dir: &Path, init_opts: i64) -> Result<i64, OcrError> {
    let model_path = engine_dir.join(MODEL_FILE);
    let model_cstr = CString::new(model_path.to_string_lossy().into_owned())?;
    let key_cstr = CString::new(MODEL_KEY.to_vec())?;

    let mut handle: i64 = 0;
    check(
        unsafe {
            (lib.create_pipeline)(model_cstr.as_ptr(), key_cstr.as_ptr(), init_opts, &mut handle)
        },
        "CreateOcrPipeline",
    )?;
    Ok(handle)
}

fn create_process_options(lib: &OcrLibrary) -> Result<i64, OcrError> {
    let mut handle: i64 = 0;
    check(
        unsafe { (lib.create_process_options)(&mut handle) },
        "CreateOcrProcessOptions",
    )?;
    check(
        unsafe { (lib.set_max_lines)(handle, 1000) },
        "SetMaxRecognitionLineCount",
    )?;
    Ok(handle)
}

// ---------------------------------------------------------------------------
// Result-parsing helpers
// ---------------------------------------------------------------------------

fn read_text(f: impl FnOnce(*mut *const c_char) -> i64) -> String {
    let mut ptr: *const c_char = std::ptr::null();
    if f(&mut ptr) == 0 && !ptr.is_null() {
        // SAFETY: the DLL owns this string and it lives until ReleaseOcrResult.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    }
}

fn read_bbox(f: impl FnOnce(*mut *const FfiBoundingBox) -> i64) -> Option<BoundingBox> {
    let mut ptr: *const FfiBoundingBox = std::ptr::null();
    if f(&mut ptr) == 0 && !ptr.is_null() {
        // SAFETY: pointer is valid until ReleaseOcrResult.
        let b = unsafe { &*ptr };
        Some(BoundingBox {
            x1: b.x1,
            y1: b.y1,
            x2: b.x2,
            y2: b.y2,
            x3: b.x3,
            y3: b.y3,
            x4: b.x4,
            y4: b.y4,
        })
    } else {
        None
    }
}

fn check(code: i64, operation: &'static str) -> Result<(), OcrError> {
    if code == 0 {
        Ok(())
    } else {
        Err(OcrError::DllCall { operation, code })
    }
}
