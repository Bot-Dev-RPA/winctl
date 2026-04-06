//! CLI integration tests using committed fixture images (no runtime dependencies).

use std::path::{Path, PathBuf};
use std::process::Command;

fn oneocr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oneocr"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// RAII guard that removes a directory on drop, even if the test panics.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// =============================================================================
// CLI flag tests
// =============================================================================

#[test]
fn help_flag() {
    let out = oneocr().arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("Extract text from images"));
    assert!(stdout.contains("--clipboard"));
    assert!(stdout.contains("--format"));
    assert!(stdout.contains("--setup"));
    assert!(stdout.contains("--copy"));
    assert!(stdout.contains("--engine-dir"));
}

#[test]
fn no_args_prints_error() {
    let out = oneocr().output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("provide image file"));
}

// =============================================================================
// Output format tests
// =============================================================================

#[test]
fn text_format_single_file() {
    let out = oneocr().arg(fixture("hello.png")).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello") && stdout.contains("World"),
        "expected 'Hello World', got: {stdout}"
    );
}

#[test]
fn text_format_is_default() {
    let out_default = oneocr().arg(fixture("hello.png")).output().unwrap();
    let out_explicit = oneocr()
        .arg(fixture("hello.png"))
        .args(["--format", "text"])
        .output()
        .unwrap();
    assert_eq!(out_default.stdout, out_explicit.stdout);
}

#[test]
fn json_format() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["-f", "json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);

    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));

    assert!(json["text"].is_string());
    assert!(json["lines"].is_array());
    let lines = json["lines"].as_array().unwrap();
    assert!(!lines.is_empty());

    let first_line = &lines[0];
    assert!(first_line["words"].is_array());
    let words = first_line["words"].as_array().unwrap();
    assert!(!words.is_empty());
    assert!(words[0]["text"].is_string());
    assert!(words[0]["confidence"].is_f64());
    assert!(words[0]["bounding_box"].is_object());
}

#[test]
fn lines_format() {
    let out = oneocr()
        .arg(fixture("multiline.png"))
        .args(["-f", "lines"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("[00]"));
    assert!(stdout.contains("Line"));
    assert!(stdout.contains("(0."));
}

#[test]
fn spaced_format() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["-f", "spaced"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello"));
}

#[test]
fn table_format_no_table() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["-f", "table"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello"));
}

// =============================================================================
// Multiple files
// =============================================================================

#[test]
fn multiple_files() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .arg(fixture("numbers.png"))
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(stdout.contains("Hello"));
    assert!(stdout.contains("12345"));
    assert!(stderr.contains("---"));
}

// =============================================================================
// Stdin
// =============================================================================

#[test]
fn stdin_input() {
    let image_bytes = std::fs::read(fixture("numbers.png")).unwrap();
    let out = oneocr()
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(&image_bytes).unwrap();
            child.wait_with_output()
        })
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("12345"), "expected '12345', got: {stdout}");
}

// =============================================================================
// --copy flag
// =============================================================================

#[test]
fn copy_flag_reports_copied() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .arg("--copy")
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("copied to clipboard"));
}

// =============================================================================
// Error cases
// =============================================================================

#[test]
fn nonexistent_file() {
    let out = oneocr().arg("this_file_does_not_exist.png").output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("loading") || stderr.contains("error") || stderr.contains("Error"),
        "expected error message, got: {stderr}"
    );
}

#[test]
fn invalid_format_flag() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["-f", "invalid_format"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn tiny_image_error() {
    let out = oneocr().arg(fixture("tiny.png")).output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("dimensions") || stderr.contains("range"),
        "expected dimension error, got: {stderr}"
    );
}

#[test]
fn clipboard_no_image() {
    let out = oneocr().arg("--clipboard").output().unwrap();
    // Should fail since clipboard likely has no image (or has text, not image)
    // This test is best-effort: if clipboard happens to have an image it will pass OCR
    // but in CI/automated contexts it will correctly fail
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("clipboard") || stderr.contains("image"),
            "expected clipboard error, got: {stderr}"
        );
    }
}

#[test]
fn engine_dir_fallthrough() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["--engine-dir", "C:\\nonexistent_dir_12345"])
        .output()
        .unwrap();
    assert!(out.status.success(), "expected fallthrough to succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Hello"));
}

// =============================================================================
// --setup flag
// =============================================================================

#[test]
fn setup_alone_succeeds() {
    let target = TempDir::new("oneocr_test_setup");

    let out = oneocr()
        .args(["--setup", "--engine-dir"])
        .arg(target.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Engine setup complete"));

    assert!(target.path().join("oneocr.dll").exists());
    assert!(target.path().join("oneocr.onemodel").exists());
    assert!(target.path().join("onnxruntime.dll").exists());
}

#[test]
fn setup_then_ocr() {
    let target = TempDir::new("oneocr_test_setup_ocr");

    let out = oneocr()
        .args(["--setup", "--engine-dir"])
        .arg(target.path())
        .arg(fixture("numbers.png"))
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("12345"), "expected '12345', got: {stdout}");
}

// =============================================================================
// JSON structure validation
// =============================================================================

#[test]
fn json_multiline_has_multiple_lines() {
    let out = oneocr()
        .arg(fixture("multiline.png"))
        .args(["-f", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let lines = json["lines"].as_array().unwrap();
    assert!(
        lines.len() >= 2,
        "expected multiple lines, got {}: {stdout}",
        lines.len()
    );
}

#[test]
fn json_bounding_box_coordinates() {
    let out = oneocr()
        .arg(fixture("hello.png"))
        .args(["-f", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let word = &json["lines"][0]["words"][0];
    let bb = &word["bounding_box"];
    for key in ["x1", "y1", "x2", "y2", "x3", "y3", "x4", "y4"] {
        let val = bb[key].as_f64().unwrap_or_else(|| panic!("missing {key}"));
        assert!(val >= 0.0, "{key} is negative: {val}");
    }
    assert!(bb["x2"].as_f64().unwrap() > bb["x1"].as_f64().unwrap());
}

#[test]
fn json_confidence_in_range() {
    let out = oneocr()
        .arg(fixture("numbers.png"))
        .args(["-f", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for line in json["lines"].as_array().unwrap() {
        for word in line["words"].as_array().unwrap() {
            let conf = word["confidence"].as_f64().unwrap();
            assert!(
                (0.0..=1.0).contains(&conf),
                "confidence {conf} out of range"
            );
        }
    }
}
