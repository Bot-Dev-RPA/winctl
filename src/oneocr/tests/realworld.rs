//! Real-world OCR tests using fixture images that simulate diverse content:
//! - Data table with grid lines
//! - Terminal log output (colored, monospace)
//! - Form layout with labels/values
//! - Dense paragraph text
//! - Code snippet

use std::path::PathBuf;
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

fn run_ocr(fixture_name: &str, format: &str) -> (String, String) {
    let out = oneocr()
        .arg(fixture(fixture_name))
        .args(["-f", format])
        .output()
        .unwrap_or_else(|e| panic!("failed to run oneocr: {e}"));
    assert!(
        out.status.success(),
        "oneocr failed on {fixture_name}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// =============================================================================
// Table with grid lines
// =============================================================================

#[test]
fn table_grid_text_extraction() {
    let (stdout, _) = run_ocr("table_grid.png", "text");
    // All headers present
    for header in ["ID", "Name", "Status", "Score"] {
        assert!(stdout.contains(header), "missing header '{header}' in:\n{stdout}");
    }
    // All data rows present
    for name in ["Alice Johnson", "Bob Smith", "Carol White", "David Brown", "Eve Martinez"] {
        assert!(stdout.contains(name), "missing name '{name}' in:\n{stdout}");
    }
    // Scores present
    for score in ["92.5", "78.3", "95.1", "84.7", "91.0"] {
        assert!(stdout.contains(score), "missing score '{score}' in:\n{stdout}");
    }
}

#[test]
fn table_grid_table_format() {
    let (stdout, _) = run_ocr("table_grid.png", "table");
    // Should produce markdown table with pipes and separators
    assert!(stdout.contains("|"), "no table pipes in:\n{stdout}");
    assert!(stdout.contains("---"), "no separator in:\n{stdout}");
    // Headers in table format
    assert!(stdout.contains("ID"), "missing ID in table");
    assert!(stdout.contains("Name"), "missing Name in table");
    // Data should be in cells
    assert!(stdout.contains("Alice Johnson"), "missing Alice Johnson in table");
    assert!(stdout.contains("92.5"), "missing 92.5 in table");
}

#[test]
fn table_grid_json_structure() {
    let (stdout, _) = run_ocr("table_grid.png", "json");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let lines = json["lines"].as_array().unwrap();
    // Table has header + 5 data rows; OCR may split cells into separate lines
    // but should have at least 6 lines total
    assert!(lines.len() >= 6, "expected >=6 lines, got {}", lines.len());
}

// =============================================================================
// Log output (monospace, timestamps, colored text)
// =============================================================================

#[test]
fn log_output_timestamps() {
    let (stdout, _) = run_ocr("log_output.png", "text");
    // Timestamps should be accurately recognized
    assert!(stdout.contains("2024-03-15"), "missing date in:\n{stdout}");
    assert!(stdout.contains("09:14:02"), "missing timestamp in:\n{stdout}");
}

#[test]
fn log_output_levels() {
    let (stdout, _) = run_ocr("log_output.png", "text");
    assert!(stdout.contains("INFO"), "missing INFO level");
    assert!(stdout.contains("WARN"), "missing WARN level");
    assert!(stdout.contains("ERROR"), "missing ERROR level");
}

#[test]
fn log_output_messages() {
    let (stdout, _) = run_ocr("log_output.png", "text");
    assert!(stdout.contains("Server started on port 8080"), "missing server start message");
    assert!(stdout.contains("Database connection established"), "missing db message");
    assert!(stdout.contains("redis://localhost"), "missing redis URL");
    assert!(stdout.contains("TLS certificate expires"), "missing TLS warning");
}

#[test]
fn log_output_line_count() {
    let (stdout, _) = run_ocr("log_output.png", "lines");
    // Should have 10 log lines (some may merge timestamp+message on one line)
    let line_markers: Vec<&str> = stdout.lines().filter(|l| l.starts_with('[')).collect();
    assert!(
        line_markers.len() >= 8,
        "expected >=8 log lines, got {}: {stdout}",
        line_markers.len()
    );
}

// =============================================================================
// Form layout (label: value pairs)
// =============================================================================

#[test]
fn form_layout_title() {
    let (stdout, _) = run_ocr("form_layout.png", "text");
    assert!(stdout.contains("Project Details"), "missing title in:\n{stdout}");
}

#[test]
fn form_layout_fields() {
    let (stdout, _) = run_ocr("form_layout.png", "text");
    // Labels
    for label in ["Project:", "Owner:", "Status:", "Priority:", "Due Date:", "Completion:"] {
        assert!(stdout.contains(label), "missing label '{label}' in:\n{stdout}");
    }
    // Values
    assert!(stdout.contains("Mercury Dashboard"), "missing project name");
    assert!(stdout.contains("Engineering Team Alpha"), "missing owner");
    assert!(stdout.contains("In Progress"), "missing status");
    assert!(stdout.contains("High"), "missing priority");
    assert!(stdout.contains("April 30, 2024"), "missing due date");
    assert!(stdout.contains("67%"), "missing completion");
}

#[test]
fn form_layout_spaced_preserves_alignment() {
    let (stdout, _) = run_ocr("form_layout.png", "spaced");
    // In spaced format, labels and values should have spacing between them
    // (not mashed together)
    assert!(stdout.contains("Project"), "missing Project in spaced output");
    assert!(stdout.contains("Mercury"), "missing Mercury in spaced output");
}

// =============================================================================
// Dense paragraph
// =============================================================================

#[test]
fn paragraph_full_text() {
    let (stdout, _) = run_ocr("paragraph.png", "text");
    // Opening and closing phrases
    assert!(
        stdout.contains("The quick brown fox jumps over the lazy dog"),
        "missing pangram in:\n{stdout}"
    );
    assert!(
        stdout.contains("exquisite opal jewels"),
        "missing ending in:\n{stdout}"
    );
}

#[test]
fn paragraph_punctuation() {
    let (stdout, _) = run_ocr("paragraph.png", "text");
    // Punctuation accuracy
    assert!(stdout.contains("alphabet."), "missing period after 'alphabet'");
    assert!(stdout.contains("jump!"), "missing exclamation mark");
    assert!(stdout.contains("vow."), "missing period after 'vow'");
}

#[test]
fn paragraph_word_count() {
    let (stdout, _) = run_ocr("paragraph.png", "json");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let total_words: usize = json["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["words"].as_array().unwrap().len())
        .sum();
    // The paragraph has ~60 words; OCR should get most of them
    assert!(
        total_words >= 50,
        "expected >=50 words, got {total_words}"
    );
}

#[test]
fn paragraph_high_confidence() {
    let (stdout, _) = run_ocr("paragraph.png", "json");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let confidences: Vec<f64> = json["lines"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["words"].as_array().unwrap())
        .map(|w| w["confidence"].as_f64().unwrap())
        .collect();
    let avg: f64 = confidences.iter().sum::<f64>() / confidences.len() as f64;
    assert!(
        avg > 0.90,
        "average confidence {avg:.3} is too low for clean rendered text"
    );
}

// =============================================================================
// Code snippet
// =============================================================================

#[test]
fn code_snippet_function_signature() {
    let (stdout, _) = run_ocr("code_snippet.png", "text");
    assert!(stdout.contains("fn calculate_score"), "missing function name in:\n{stdout}");
    assert!(stdout.contains("f64"), "missing return type in:\n{stdout}");
}

#[test]
fn code_snippet_keywords() {
    let (stdout, _) = run_ocr("code_snippet.png", "text");
    for kw in ["let", "if", "else", "fn"] {
        assert!(stdout.contains(kw), "missing keyword '{kw}' in:\n{stdout}");
    }
}

#[test]
fn code_snippet_method_calls() {
    let (stdout, _) = run_ocr("code_snippet.png", "text");
    assert!(stdout.contains(".filter("), "missing .filter() call");
    assert!(stdout.contains(".map("), "missing .map() call");
    assert!(stdout.contains(".sum()"), "missing .sum() call");
}

#[test]
fn code_snippet_comment() {
    let (stdout, _) = run_ocr("code_snippet.png", "text");
    assert!(
        stdout.contains("// Filter active items"),
        "missing comment in:\n{stdout}"
    );
}

#[test]
fn code_snippet_special_chars() {
    let (stdout, _) = run_ocr("code_snippet.png", "text");
    // Code has special characters that are tricky for OCR
    assert!(stdout.contains("&[Item]"), "missing &[Item] in:\n{stdout}");
    assert!(stdout.contains("i.value"), "missing i.value in:\n{stdout}");
}

// =============================================================================
// Cross-format consistency
// =============================================================================

#[test]
fn all_formats_produce_output() {
    for fixture_name in ["table_grid.png", "log_output.png", "form_layout.png", "paragraph.png", "code_snippet.png"] {
        for format in ["text", "json", "lines", "spaced", "table"] {
            let (stdout, _) = run_ocr(fixture_name, format);
            assert!(
                !stdout.trim().is_empty(),
                "{fixture_name} with format '{format}' produced empty output"
            );
        }
    }
}
