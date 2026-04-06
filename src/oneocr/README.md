# oneocr - source

Offline OCR CLI using Windows Snipping Tool's engine via FFI.

## Build

Requires Rust 1.82+ and Windows 11 with Snipping Tool 11.2409+.

```bash
cd src/oneocr
cargo build --release
cp target/release/oneocr.exe ../../skills/oneocr/bin/
```

## Engine Files

The OCR engine files (~110MB) are auto-copied from Snipping Tool on first run. To use a custom location:

```bash
oneocr --engine-dir /path/to/engine image.png
# or
ONEOCR_ENGINE_DIR=/path/to/engine oneocr image.png
```

## Requirements

- Windows 11 with Snipping Tool 11.2409+
- Rust 1.82+ (build only)
