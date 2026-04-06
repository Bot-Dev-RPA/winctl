---
name: screenshot
description: Capture screenshots of windows, screens, monitors, or regions. Use when the user asks to take a screenshot, capture a window, grab the screen, or save what's on screen to an image.
argument-hint: <window-title> | --screen | --monitor N | --region x,y,w,h
allowed-tools: Bash
---

# Screenshot

Capture screenshots using the `Screenshot` CLI tool.

## Tool

```
EXE="${CLAUDE_SKILL_DIR}/bin/Screenshot.exe"
```

Run via Bash: `"${CLAUDE_SKILL_DIR}/bin/Screenshot.exe" <args>`

## Capture Targets

Exactly one required:
- `--title "partial name"` -- capture a window by title (case-insensitive partial match)
- `--pid 1234` -- capture a window by process ID
- `--screen` -- capture the entire screen (all monitors)
- `--monitor N` -- capture a specific monitor (1-based)
- `--region x,y,w,h` -- capture a rectangular region

## Output

- `--output path.png` -- output file path (default: `screenshot.png` in cwd)
- Always PNG format
- Prints the absolute path of the saved file to stdout

## Examples

```bash
"$EXE" --title "Chrome" --output chrome.png
"$EXE" --screen --output fullscreen.png
"$EXE" --monitor 2 --output monitor2.png
"$EXE" --region 100,100,800,600 --output region.png
```

## Parsing natural language from $ARGUMENTS

| User says | Command |
|---|---|
| `screenshot Chrome` | `--title "Chrome" --output screenshot.png` |
| `capture the screen` | `--screen` |
| `screenshot monitor 2` | `--monitor 2` |
| `grab region 0,0,800,600` | `--region 0,0,800,600` |

## Notes

- Title matching is case-insensitive and partial
- Minimized windows are auto-restored before capture
- If window not found, suggest the user run `winctl list` to see available windows
- Output defaults to `screenshot.png` in the current directory
- Always tell the user the output file path after capture
