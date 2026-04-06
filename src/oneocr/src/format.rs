use crate::types::OcrResult;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A detected table with cells indexed by [row][column].
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: usize,
    pub columns: usize,
    /// Cell text indexed as `cells[row][column]`.
    pub cells: Vec<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Spaced text reconstruction
// ---------------------------------------------------------------------------

/// Reconstruct horizontal spacing from word bounding boxes.
///
/// Groups OCR lines into visual rows by y-overlap, then uses pixel
/// positions to insert proportional spaces between words.
pub fn to_spaced_text(result: &OcrResult) -> String {
    let words = collect_words(result);
    if words.is_empty() {
        return result.text.clone();
    }

    let char_width = estimate_char_width(&words);
    if char_width <= 0.0 {
        return result.text.clone();
    }

    let rows = group_into_visual_rows(&words);
    let mut output = String::new();

    for row in &rows {
        let indent = (row[0].left / char_width).round() as usize;
        output.extend(std::iter::repeat_n(' ', indent));

        for (i, word) in row.iter().enumerate() {
            if i > 0 {
                let gap_px = word.left - row[i - 1].right;
                let gap_chars = (gap_px / char_width).round().max(1.0) as usize;
                output.extend(std::iter::repeat_n(' ', gap_chars));
            }
            output.push_str(&word.text);
        }
        output.push('\n');
    }

    // Trim trailing newline
    if output.ends_with('\n') {
        output.pop();
    }
    output
}

// ---------------------------------------------------------------------------
// Table detection -- replicated from SnippingToolUI.dll TableProcessor
// ---------------------------------------------------------------------------

/// Detect tables in OCR results using the Snipping Tool algorithm.
///
/// Algorithm (reverse-engineered from SnippingToolUI.dll `FUN_1800a6c50`):
/// 1. For each line, compute y-range and x-range from bounding box
/// 2. Merge into row boundaries by y-overlap; merge into column boundaries by x-overlap
/// 3. Sort and post-merge overlapping boundaries
/// 4. If >= 2 rows AND >= 2 columns, assign each line to a (row, col) cell
pub fn detect_tables(result: &OcrResult) -> Vec<Table> {
    let words = collect_words(result);
    if words.is_empty() {
        return Vec::new();
    }

    // Phase 1: compute row and column boundaries via interval merging
    let mut row_bounds: Vec<[f32; 2]> = Vec::new();
    let mut col_bounds: Vec<[f32; 2]> = Vec::new();

    for line in &result.lines {
        let Some(bbox) = &line.bounding_box else {
            continue;
        };
        let y_start = bbox.top();
        let y_end = bbox.bottom();
        let x_start = bbox.left();
        let x_end = bbox.right();

        merge_interval(&mut row_bounds, y_start, y_end);
        merge_interval(&mut col_bounds, x_start, x_end);
    }

    // Sort by start coordinate
    row_bounds.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    col_bounds.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));

    // Post-sort merge: adjacent overlapping boundaries
    post_merge(&mut row_bounds);
    post_merge(&mut col_bounds);

    // Minimum 2x2 grid required (matches SnippingToolUI check at FUN_1800819d0)
    if row_bounds.len() < 2 || col_bounds.len() < 2 {
        return Vec::new();
    }

    // Phase 2: assign each line to a (row, column) cell
    let num_rows = row_bounds.len();
    let num_cols = col_bounds.len();
    let mut cells: Vec<Vec<Vec<String>>> = vec![vec![Vec::new(); num_cols]; num_rows];

    for line in &result.lines {
        let Some(bbox) = &line.bounding_box else {
            continue;
        };
        let y_mid = (bbox.top() + bbox.bottom()) / 2.0;
        let x_mid = (bbox.left() + bbox.right()) / 2.0;

        let row_idx = find_interval(&row_bounds, y_mid);
        let col_idx = find_interval(&col_bounds, x_mid);

        if let (Some(r), Some(c)) = (row_idx, col_idx) {
            cells[r][c].push(line.text.clone());
        }
    }

    let merged_cells: Vec<Vec<String>> = cells
        .into_iter()
        .map(|row| row.into_iter().map(|texts| texts.join(" ")).collect())
        .collect();

    vec![Table {
        rows: num_rows,
        columns: num_cols,
        cells: merged_cells,
    }]
}

/// Format detected tables as a markdown table string.
pub fn to_table_text(result: &OcrResult) -> String {
    let tables = detect_tables(result);
    if tables.is_empty() {
        return result.text.clone();
    }

    let mut output = String::new();
    for (ti, table) in tables.iter().enumerate() {
        if ti > 0 {
            output.push('\n');
        }

        // Calculate column widths
        let mut col_widths: Vec<usize> = vec![0; table.columns];
        for row in &table.cells {
            for (c, cell) in row.iter().enumerate() {
                col_widths[c] = col_widths[c].max(cell.len());
            }
        }
        // Minimum width of 3 for the separator
        for w in &mut col_widths {
            *w = (*w).max(3);
        }

        // Render rows
        for (r, row) in table.cells.iter().enumerate() {
            output.push('|');
            for (c, cell) in row.iter().enumerate() {
                output.push(' ');
                output.push_str(cell);
                let padding = col_widths[c] - cell.len();
                output.extend(std::iter::repeat_n(' ', padding));
                output.push_str(" |");
            }
            output.push('\n');

            // Separator after first row (header)
            if r == 0 {
                output.push('|');
                for &w in &col_widths {
                    output.push(' ');
                    output.extend(std::iter::repeat_n('-', w));
                    output.push_str(" |");
                }
                output.push('\n');
            }
        }
    }

    if output.ends_with('\n') {
        output.pop();
    }
    output
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct PositionedWord {
    text: String,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

fn collect_words(result: &OcrResult) -> Vec<PositionedWord> {
    let mut words = Vec::new();
    for line in &result.lines {
        for word in &line.words {
            if let Some(bbox) = &word.bounding_box {
                words.push(PositionedWord {
                    text: word.text.clone(),
                    left: bbox.left(),
                    right: bbox.right(),
                    top: bbox.top(),
                    bottom: bbox.bottom(),
                });
            }
        }
    }
    words
}

fn estimate_char_width(words: &[PositionedWord]) -> f32 {
    let mut total_px: f32 = 0.0;
    let mut total_chars: usize = 0;
    for w in words {
        let width = w.right - w.left;
        if width > 0.0 && !w.text.is_empty() {
            total_px += width;
            total_chars += w.text.len();
        }
    }
    if total_chars == 0 {
        return 0.0;
    }
    total_px / total_chars as f32
}

/// Group words into visual rows by y-coordinate overlap.
fn group_into_visual_rows(words: &[PositionedWord]) -> Vec<Vec<&PositionedWord>> {
    if words.is_empty() {
        return Vec::new();
    }

    // Build row boundaries using the same interval-merge approach as table detection
    let mut row_bounds: Vec<[f32; 2]> = Vec::new();
    for w in words {
        merge_interval(&mut row_bounds, w.top, w.bottom);
    }
    row_bounds.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    post_merge(&mut row_bounds);

    // Assign words to rows
    let mut rows: Vec<Vec<&PositionedWord>> = vec![Vec::new(); row_bounds.len()];
    for w in words {
        let y_mid = (w.top + w.bottom) / 2.0;
        if let Some(idx) = find_interval(&row_bounds, y_mid) {
            rows[idx].push(w);
        }
    }

    // Sort words within each row by x position
    for row in &mut rows {
        row.sort_by(|a, b| a.left.partial_cmp(&b.left).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Remove empty rows
    rows.retain(|r| !r.is_empty());
    rows
}

/// Merge a new interval [start, end] into the boundary list.
/// If it overlaps an existing boundary, expand it. Otherwise append.
/// (Replicates FUN_1800a6c50 lines 1411-1443 / 1471-1504)
fn merge_interval(bounds: &mut Vec<[f32; 2]>, start: f32, end: f32) {
    for b in bounds.iter_mut() {
        // Check overlap: b.start < end && start < b.end
        if b[0] < end && start < b[1] {
            // Merge: expand to union
            b[0] = b[0].min(start);
            b[1] = b[1].max(end);
            return;
        }
    }
    bounds.push([start, end]);
}

/// After sorting, merge any adjacent boundaries that overlap.
/// (Replicates FUN_1800a6c50 lines 1621-1689)
fn post_merge(bounds: &mut Vec<[f32; 2]>) {
    let mut i = 0;
    while i + 1 < bounds.len() {
        // If current overlaps with next
        if bounds[i][0] < bounds[i + 1][1] && bounds[i + 1][0] < bounds[i][1] {
            // Merge: expand current to cover both
            bounds[i][0] = bounds[i][0].min(bounds[i + 1][0]);
            bounds[i][1] = bounds[i][1].max(bounds[i + 1][1]);
            bounds.remove(i + 1);
            // Don't advance i -- check if the merged interval overlaps the new i+1
        } else {
            i += 1;
        }
    }
}

/// Find which interval contains the given point.
fn find_interval(bounds: &[[f32; 2]], point: f32) -> Option<usize> {
    bounds
        .iter()
        .position(|b| b[0] <= point && point <= b[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BoundingBox, OcrLine, OcrResult, OcrWord};

    fn make_bbox(x1: f32, y1: f32, x2: f32, y2: f32) -> Option<BoundingBox> {
        // Axis-aligned rectangle: TL(x1,y1) TR(x2,y1) BR(x2,y2) BL(x1,y2)
        Some(BoundingBox {
            x1, y1, x2, y2: y1, x3: x2, y3: y2, x4: x1, y4: y2,
        })
    }

    fn make_word(text: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> OcrWord {
        OcrWord {
            text: text.to_string(),
            bounding_box: make_bbox(x1, y1, x2, y2),
            confidence: 0.99,
        }
    }

    fn make_line(text: &str, words: Vec<OcrWord>, x1: f32, y1: f32, x2: f32, y2: f32) -> OcrLine {
        OcrLine {
            text: text.to_string(),
            bounding_box: make_bbox(x1, y1, x2, y2),
            words,
        }
    }

    fn make_result(lines: Vec<OcrLine>) -> OcrResult {
        let text = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        OcrResult { text, text_angle: None, lines }
    }

    // -- merge_interval -------------------------------------------------------

    #[test]
    fn merge_interval_adds_new() {
        let mut bounds = Vec::new();
        merge_interval(&mut bounds, 10.0, 20.0);
        assert_eq!(bounds, vec![[10.0, 20.0]]);
    }

    #[test]
    fn merge_interval_no_overlap() {
        let mut bounds = vec![[10.0, 20.0]];
        merge_interval(&mut bounds, 30.0, 40.0);
        assert_eq!(bounds.len(), 2);
    }

    #[test]
    fn merge_interval_with_overlap() {
        let mut bounds = vec![[10.0, 25.0]];
        merge_interval(&mut bounds, 20.0, 35.0);
        assert_eq!(bounds, vec![[10.0, 35.0]]);
    }

    #[test]
    fn merge_interval_contained() {
        let mut bounds = vec![[10.0, 40.0]];
        merge_interval(&mut bounds, 15.0, 25.0);
        assert_eq!(bounds, vec![[10.0, 40.0]]);
    }

    // -- post_merge -----------------------------------------------------------

    #[test]
    fn post_merge_overlapping() {
        let mut bounds = vec![[10.0, 25.0], [20.0, 35.0], [50.0, 60.0]];
        post_merge(&mut bounds);
        assert_eq!(bounds, vec![[10.0, 35.0], [50.0, 60.0]]);
    }

    #[test]
    fn post_merge_no_overlap() {
        let mut bounds = vec![[10.0, 20.0], [30.0, 40.0]];
        post_merge(&mut bounds);
        assert_eq!(bounds, vec![[10.0, 20.0], [30.0, 40.0]]);
    }

    #[test]
    fn post_merge_chain() {
        // Three intervals that chain-merge into one
        let mut bounds = vec![[10.0, 25.0], [20.0, 35.0], [30.0, 50.0]];
        post_merge(&mut bounds);
        assert_eq!(bounds, vec![[10.0, 50.0]]);
    }

    #[test]
    fn post_merge_empty() {
        let mut bounds: Vec<[f32; 2]> = Vec::new();
        post_merge(&mut bounds);
        assert!(bounds.is_empty());
    }

    // -- find_interval --------------------------------------------------------

    #[test]
    fn find_interval_inside() {
        let bounds = vec![[10.0, 20.0], [30.0, 40.0]];
        assert_eq!(find_interval(&bounds, 15.0), Some(0));
        assert_eq!(find_interval(&bounds, 35.0), Some(1));
    }

    #[test]
    fn find_interval_at_boundary() {
        let bounds = vec![[10.0, 20.0]];
        assert_eq!(find_interval(&bounds, 10.0), Some(0));
        assert_eq!(find_interval(&bounds, 20.0), Some(0));
    }

    #[test]
    fn find_interval_outside() {
        let bounds = vec![[10.0, 20.0], [30.0, 40.0]];
        assert_eq!(find_interval(&bounds, 5.0), None);
        assert_eq!(find_interval(&bounds, 25.0), None);
        assert_eq!(find_interval(&bounds, 45.0), None);
    }

    // -- estimate_char_width --------------------------------------------------

    #[test]
    fn char_width_normal() {
        let words = vec![
            PositionedWord { text: "Hello".into(), left: 10.0, right: 60.0, top: 0.0, bottom: 20.0 },
            PositionedWord { text: "World".into(), left: 70.0, right: 120.0, top: 0.0, bottom: 20.0 },
        ];
        // Total px = 50 + 50 = 100, total chars = 5 + 5 = 10
        assert_eq!(estimate_char_width(&words), 10.0);
    }

    #[test]
    fn char_width_empty() {
        let words: Vec<PositionedWord> = Vec::new();
        assert_eq!(estimate_char_width(&words), 0.0);
    }

    #[test]
    fn char_width_zero_width_word_ignored() {
        let words = vec![
            PositionedWord { text: "Hi".into(), left: 10.0, right: 30.0, top: 0.0, bottom: 20.0 },
            PositionedWord { text: "".into(), left: 40.0, right: 40.0, top: 0.0, bottom: 20.0 },
        ];
        // Only "Hi" counts: 20px / 2 chars = 10
        assert_eq!(estimate_char_width(&words), 10.0);
    }

    // -- to_spaced_text -------------------------------------------------------

    #[test]
    fn spaced_text_single_line() {
        let result = make_result(vec![
            make_line("Hello World", vec![
                make_word("Hello", 0.0, 0.0, 50.0, 20.0),
                make_word("World", 70.0, 0.0, 120.0, 20.0),
            ], 0.0, 0.0, 120.0, 20.0),
        ]);
        let out = to_spaced_text(&result);
        assert!(out.contains("Hello"));
        assert!(out.contains("World"));
    }

    #[test]
    fn spaced_text_empty_result() {
        let result = OcrResult {
            text: String::new(),
            text_angle: None,
            lines: Vec::new(),
        };
        assert_eq!(to_spaced_text(&result), "");
    }

    #[test]
    fn spaced_text_no_bboxes_falls_back() {
        let result = OcrResult {
            text: "fallback text".into(),
            text_angle: None,
            lines: vec![OcrLine {
                text: "fallback text".into(),
                bounding_box: None,
                words: vec![OcrWord {
                    text: "fallback".into(),
                    bounding_box: None,
                    confidence: 0.99,
                }],
            }],
        };
        assert_eq!(to_spaced_text(&result), "fallback text");
    }

    #[test]
    fn spaced_text_two_rows() {
        let result = make_result(vec![
            make_line("Top", vec![
                make_word("Top", 10.0, 0.0, 40.0, 20.0),
            ], 10.0, 0.0, 40.0, 20.0),
            make_line("Bottom", vec![
                make_word("Bottom", 10.0, 30.0, 70.0, 50.0),
            ], 10.0, 30.0, 70.0, 50.0),
        ]);
        let out = to_spaced_text(&result);
        let output_lines: Vec<&str> = out.lines().collect();
        assert_eq!(output_lines.len(), 2);
        assert!(output_lines[0].contains("Top"));
        assert!(output_lines[1].contains("Bottom"));
    }

    // -- detect_tables --------------------------------------------------------

    #[test]
    fn table_empty_result() {
        let result = make_result(vec![]);
        assert!(detect_tables(&result).is_empty());
    }

    #[test]
    fn table_single_line_no_table() {
        let result = make_result(vec![
            make_line("just one line", vec![
                make_word("just", 10.0, 10.0, 50.0, 30.0),
                make_word("one", 60.0, 10.0, 90.0, 30.0),
                make_word("line", 100.0, 10.0, 140.0, 30.0),
            ], 10.0, 10.0, 140.0, 30.0),
        ]);
        // Single row can't form a table (needs >= 2x2)
        assert!(detect_tables(&result).is_empty());
    }

    #[test]
    fn table_2x2_grid() {
        // 4 lines arranged in a 2x2 grid
        let result = make_result(vec![
            make_line("A", vec![make_word("A", 10.0, 10.0, 50.0, 30.0)], 10.0, 10.0, 50.0, 30.0),
            make_line("B", vec![make_word("B", 200.0, 10.0, 240.0, 30.0)], 200.0, 10.0, 240.0, 30.0),
            make_line("C", vec![make_word("C", 10.0, 100.0, 50.0, 120.0)], 10.0, 100.0, 50.0, 120.0),
            make_line("D", vec![make_word("D", 200.0, 100.0, 240.0, 120.0)], 200.0, 100.0, 240.0, 120.0),
        ]);
        let tables = detect_tables(&result);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.rows, 2);
        assert_eq!(t.columns, 2);
        assert_eq!(t.cells[0][0], "A");
        assert_eq!(t.cells[0][1], "B");
        assert_eq!(t.cells[1][0], "C");
        assert_eq!(t.cells[1][1], "D");
    }

    // -- to_table_text --------------------------------------------------------

    #[test]
    fn table_text_markdown_format() {
        let result = make_result(vec![
            make_line("Name", vec![make_word("Name", 10.0, 10.0, 60.0, 30.0)], 10.0, 10.0, 60.0, 30.0),
            make_line("Age", vec![make_word("Age", 200.0, 10.0, 240.0, 30.0)], 200.0, 10.0, 240.0, 30.0),
            make_line("Alice", vec![make_word("Alice", 10.0, 100.0, 60.0, 120.0)], 10.0, 100.0, 60.0, 120.0),
            make_line("30", vec![make_word("30", 200.0, 100.0, 230.0, 120.0)], 200.0, 100.0, 230.0, 120.0),
        ]);
        let out = to_table_text(&result);
        assert!(out.contains("| Name"));
        assert!(out.contains("| Age"));
        assert!(out.contains("| Alice"));
        assert!(out.contains("| 30"));
        assert!(out.contains("---")); // separator
    }

    #[test]
    fn table_text_no_table_falls_back() {
        let result = make_result(vec![
            make_line("no table", vec![
                make_word("no", 10.0, 10.0, 30.0, 30.0),
                make_word("table", 40.0, 10.0, 80.0, 30.0),
            ], 10.0, 10.0, 80.0, 30.0),
        ]);
        assert_eq!(to_table_text(&result), "no table");
    }
}
