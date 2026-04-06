use std::ffi::c_char;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use libloading::{Library, Symbol};

use crate::error::OcrError;

// ---------------------------------------------------------------------------
// repr(C) types matching the DLL's ABI
// ---------------------------------------------------------------------------

#[repr(C)]
pub(crate) struct FfiImage {
    pub image_type: i32,
    pub width: i32,
    pub height: i32,
    pub _reserved: i32,
    pub step_size: i64,
    pub data_ptr: *const u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct FfiBoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub x3: f32,
    pub y3: f32,
    pub x4: f32,
    pub y4: f32,
}

// ---------------------------------------------------------------------------
// Function-pointer type aliases for every oneocr.dll export we use
// ---------------------------------------------------------------------------

type FnCreateInitOpts = unsafe extern "C" fn(*mut i64) -> i64;
type FnSetDelayLoad = unsafe extern "C" fn(i64, c_char) -> i64;
type FnCreatePipeline = unsafe extern "C" fn(*const c_char, *const c_char, i64, *mut i64) -> i64;
type FnCreateProcOpts = unsafe extern "C" fn(*mut i64) -> i64;
type FnSetMaxLines = unsafe extern "C" fn(i64, i64) -> i64;
type FnRunPipeline = unsafe extern "C" fn(i64, *const FfiImage, i64, *mut i64) -> i64;

type FnGetAngle = unsafe extern "C" fn(i64, *mut f32) -> i64;
type FnGetLineCount = unsafe extern "C" fn(i64, *mut i64) -> i64;
type FnGetLine = unsafe extern "C" fn(i64, i64, *mut i64) -> i64;
type FnGetLineContent = unsafe extern "C" fn(i64, *mut *const c_char) -> i64;
type FnGetLineBBox = unsafe extern "C" fn(i64, *mut *const FfiBoundingBox) -> i64;
type FnGetWordCount = unsafe extern "C" fn(i64, *mut i64) -> i64;
type FnGetWord = unsafe extern "C" fn(i64, i64, *mut i64) -> i64;
type FnGetWordContent = unsafe extern "C" fn(i64, *mut *const c_char) -> i64;
type FnGetWordBBox = unsafe extern "C" fn(i64, *mut *const FfiBoundingBox) -> i64;
type FnGetWordConf = unsafe extern "C" fn(i64, *mut f32) -> i64;

type FnRelease = unsafe extern "C" fn(i64);

// ---------------------------------------------------------------------------
// Loaded library holding resolved function pointers
// ---------------------------------------------------------------------------

pub(crate) struct OcrLibrary {
    _lib: Library,

    // lifecycle
    pub create_init_options: FnCreateInitOpts,
    pub set_delay_load: FnSetDelayLoad,
    pub create_pipeline: FnCreatePipeline,
    pub create_process_options: FnCreateProcOpts,
    pub set_max_lines: FnSetMaxLines,

    // execution
    pub run_pipeline: FnRunPipeline,

    // result access
    pub get_angle: FnGetAngle,
    pub get_line_count: FnGetLineCount,
    pub get_line: FnGetLine,
    pub get_line_content: FnGetLineContent,
    pub get_line_bbox: FnGetLineBBox,
    pub get_word_count: FnGetWordCount,
    pub get_word: FnGetWord,
    pub get_word_content: FnGetWordContent,
    pub get_word_bbox: FnGetWordBBox,
    pub get_word_confidence: FnGetWordConf,

    // cleanup
    pub release_result: FnRelease,
    pub release_init_options: FnRelease,
    pub release_pipeline: FnRelease,
    pub release_process_options: FnRelease,
}

impl OcrLibrary {
    /// Load `oneocr.dll` from `engine_dir` and resolve every export we need.
    pub fn load(engine_dir: &Path) -> Result<Self, OcrError> {
        // Point the DLL search path at engine_dir so onnxruntime.dll is found.
        set_dll_directory(engine_dir)?;

        let dll_path = engine_dir.join("oneocr.dll");

        // SAFETY: We load a native DLL, resolve symbols into raw function
        // pointers (which are Copy), then keep the Library alive in `_lib`
        // so the pointers remain valid for the struct's lifetime.
        unsafe {
            let lib = Library::new(dll_path.as_os_str());

            // Reset DLL search path so we don't affect other DLL loads in
            // the process (matters when oneocr is used as a library).
            reset_dll_directory();

            let lib = lib?;

            macro_rules! sym {
                ($name:literal, $ty:ty) => {{
                    let s: Symbol<$ty> = lib
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|_| OcrError::MissingSymbol { name: $name })?;
                    *s
                }};
            }

            let create_init_options = sym!("CreateOcrInitOptions", FnCreateInitOpts);
            let set_delay_load =
                sym!("OcrInitOptionsSetUseModelDelayLoad", FnSetDelayLoad);
            let create_pipeline = sym!("CreateOcrPipeline", FnCreatePipeline);
            let create_process_options = sym!("CreateOcrProcessOptions", FnCreateProcOpts);
            let set_max_lines =
                sym!("OcrProcessOptionsSetMaxRecognitionLineCount", FnSetMaxLines);
            let run_pipeline = sym!("RunOcrPipeline", FnRunPipeline);

            let get_angle = sym!("GetImageAngle", FnGetAngle);
            let get_line_count = sym!("GetOcrLineCount", FnGetLineCount);
            let get_line = sym!("GetOcrLine", FnGetLine);
            let get_line_content = sym!("GetOcrLineContent", FnGetLineContent);
            let get_line_bbox = sym!("GetOcrLineBoundingBox", FnGetLineBBox);
            let get_word_count = sym!("GetOcrLineWordCount", FnGetWordCount);
            let get_word = sym!("GetOcrWord", FnGetWord);
            let get_word_content = sym!("GetOcrWordContent", FnGetWordContent);
            let get_word_bbox = sym!("GetOcrWordBoundingBox", FnGetWordBBox);
            let get_word_confidence = sym!("GetOcrWordConfidence", FnGetWordConf);

            let release_result = sym!("ReleaseOcrResult", FnRelease);
            let release_init_options = sym!("ReleaseOcrInitOptions", FnRelease);
            let release_pipeline = sym!("ReleaseOcrPipeline", FnRelease);
            let release_process_options = sym!("ReleaseOcrProcessOptions", FnRelease);

            Ok(Self {
                _lib: lib,
                create_init_options,
                set_delay_load,
                create_pipeline,
                create_process_options,
                set_max_lines,
                run_pipeline,
                get_angle,
                get_line_count,
                get_line,
                get_line_content,
                get_line_bbox,
                get_word_count,
                get_word,
                get_word_content,
                get_word_bbox,
                get_word_confidence,
                release_result,
                release_init_options,
                release_pipeline,
                release_process_options,
            })
        }
    }
}

/// Call `kernel32!SetDllDirectoryW` so that dependent DLLs (onnxruntime.dll)
/// are found when we load oneocr.dll.
fn set_dll_directory(dir: &Path) -> Result<(), OcrError> {
    type SetDllDirectoryW = unsafe extern "system" fn(*const u16) -> i32;

    // SAFETY: kernel32 is always present; we call a well-known Win32 API.
    unsafe {
        let kernel32 = Library::new("kernel32.dll")?;
        let func: Symbol<SetDllDirectoryW> = kernel32.get(b"SetDllDirectoryW\0")?;

        let wide: Vec<u16> = dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        func(wide.as_ptr());
    }
    Ok(())
}

/// Reset DLL search path to default so we don't leak a process-wide side effect.
fn reset_dll_directory() {
    type SetDllDirectoryW = unsafe extern "system" fn(*const u16) -> i32;

    unsafe {
        if let Ok(kernel32) = Library::new("kernel32.dll") {
            if let Ok(func) = kernel32.get::<SetDllDirectoryW>(b"SetDllDirectoryW\0") {
                func(std::ptr::null());
            }
        }
    }
}
