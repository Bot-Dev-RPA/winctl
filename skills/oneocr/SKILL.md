---
name: oneocr
description: Extract text from images using offline OCR. Use when the user asks to read text from an image, screenshot, or clipboard, or when you need to extract text from a visual.
argument-hint: <image-path> | --clipboard | - (stdin)
allowed-tools: Bash
---

# oneocr

Offline OCR using Windows Snipping Tool's engine. Extracts text from images with no API keys or network required.

## Tool

```
EXE="${CLAUDE_SKILL_DIR}/bin/oneocr.exe"
```

Run via Bash: `"${CLAUDE_SKILL_DIR}/bin/oneocr.exe" <args>`

## First Run

The engine files (~110MB) are auto-copied from Windows Snipping Tool on first use. Requires Windows 11 with Snipping Tool 11.2409+.

If engine files are already available elsewhere, pass `--engine-dir <path>` or set `ONEOCR_ENGINE_DIR`.

## Capture Targets

- `<file>` -- one or more image files (PNG, JPEG, BMP, GIF, TIFF, WebP)
- `--clipboard` / `-c` -- read image from clipboard
- `-` -- read image from stdin

## Output Formats

- `--format text` (default) -- plain extracted text
- `--format json` / `-f json` -- structured JSON with bounding boxes and confidence
- `--format lines` / `-f lines` -- per-line with index and confidence
- `--format spaced` / `-f spaced` -- reconstructs horizontal spacing for column-aligned text
- `--format table` / `-f table` -- detects tabular layouts, outputs as Markdown

## Other Flags

- `--copy` -- copy extracted text to clipboard
- `--engine-dir <path>` -- path to engine directory

## Examples

```bash
"$EXE" photo.png
"$EXE" screenshot.png -f json
"$EXE" --clipboard
"$EXE" document.png --copy
"$EXE" scan1.png scan2.jpg scan3.bmp
cat image.png | "$EXE" -
"$EXE" table.png -f table
```

## Parsing natural language from $ARGUMENTS

| User says | Command |
|---|---|
| `ocr screenshot.png` | `screenshot.png` |
| `read text from image.png` | `image.png` |
| `ocr clipboard` | `--clipboard` |
| `extract text as json` | `<file> -f json` |
| `ocr this table` | `<file> -f table` |

## Notes

- Supports images from 50x50 to 10,000x10,000 pixels
- Engine files are cached next to the executable after first run
- For structured data extraction, prefer `-f json` for per-word bounding boxes
- For tabular content, use `-f table` for Markdown output
- If the user provides a screenshot path, just OCR it directly
