---
name: winctl
description: Resize, move, snap, maximize, minimize, center, pin, or list windows. Use when the user asks to resize a window, set dimensions, move, arrange, snap, list open windows, or query window/monitor info.
argument-hint: <window-title> <width>x<height> [at x,y] | list | snap left/right | maximize | center | monitor-info
allowed-tools: Bash
---

# winctl

Manage desktop windows using the `winctl` CLI tool.

## Tool

```
EXE="${CLAUDE_SKILL_DIR}/bin/winctl.exe"
```

Run via Bash: `"${CLAUDE_SKILL_DIR}/bin/winctl.exe" <args>`

## Targeting a window

Two ways to target:
- `--title "partial name"` (case-insensitive partial match)
- `--pid 1234` (by process ID, useful when multiple windows share titles)

## Commands

### Resize / Move

```bash
"$EXE" --title "Chrome" --width 1920 --height 1080
"$EXE" --title "Chrome" --x 0 --y 0
"$EXE" --title "Chrome" --width 800 --height 600 --x 100 --y 100
```

### Window state

```bash
"$EXE" --title "Chrome" --maximize
"$EXE" --title "Chrome" --minimize
"$EXE" --title "Chrome" --restore
```

### Center on monitor

```bash
"$EXE" --title "Chrome" --center
"$EXE" --title "Chrome" --center --width 1280 --height 720   # resize + center
```

### Snap to half

```bash
"$EXE" --title "Chrome" --snap left
"$EXE" --title "VS Code" --snap right
"$EXE" --title "Terminal" --snap top
"$EXE" --title "Browser" --snap bottom
```

### Pin on top / unpin

```bash
"$EXE" --title "Notepad" --topmost
"$EXE" --title "Notepad" --no-topmost
```

### Move to monitor

```bash
"$EXE" --title "Chrome" --monitor 2
"$EXE" --title "Chrome" --monitor 1 --snap left   # move to monitor 1, then snap left
```

### Query window info (JSON)

```bash
"$EXE" --title "Chrome" --info
```

Returns: title, process, pid, width, height, x, y, state, monitor, topmost.

### List windows

```bash
"$EXE" list                              # table format
"$EXE" list --json                       # structured JSON
"$EXE" list --json --filter "Chrome"     # filter by title
"$EXE" list --filter "Code"              # table, filtered
```

### List monitors (JSON)

```bash
"$EXE" monitor-info
```

Returns: number, device, resolution, position, work area, primary flag.

### Wait for a window to appear

```bash
"$EXE" wait-for --title "Notepad" --timeout 10
```

Polls every 500ms until the window appears or timeout (default 30s). Returns window info as JSON on success.

## Parsing natural language from $ARGUMENTS

| User says | Command |
|---|---|
| `Chrome 1920x1080` | `--title "Chrome" --width 1920 --height 1080` |
| `Notepad 800x600 at 0,0` | `--title "Notepad" --width 800 --height 600 --x 0 --y 0` |
| `snap Chrome left` | `--title "Chrome" --snap left` |
| `center Firefox 1280x720` | `--title "Firefox" --center --width 1280 --height 720` |
| `maximize VS Code` | `--title "VS Code" --maximize` |
| `pin Notepad on top` | `--title "Notepad" --topmost` |
| `move Chrome to monitor 2` | `--title "Chrome" --monitor 2` |
| `list` | `list --json` |
| `list chrome` | `list --json --filter "chrome"` |
| `monitors` | `monitor-info` |

## Notes

- Title matching is case-insensitive and partial
- Minimized windows are auto-restored before resize/move/snap/center
- If window not found, run `list --json` and show the user available windows
- Prefer `--json` output when Claude needs to reason about results
- Flags can be combined: `--monitor 2 --snap left --topmost`
