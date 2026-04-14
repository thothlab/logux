# logux

[![ru](https://img.shields.io/badge/lang-Русский-green)](README.md)

**Android Logs & Traffic CLI** -- a TUI tool for Android developers: real-time log viewing with columnar layout, filtering, traffic inspection, and network response mocking.

---

## Features

- **Split-screen TUI** -- logs scroll on top, input line always visible at the bottom
- **Columnar log output** -- timestamp, level, tag, message in fixed-width columns; long messages wrap within the message column
- **ADB Logs** -- reads `adb logcat` with colored, formatted output
- **Smart Filtering** -- by package, tag, level, PID, regex, text -- all changeable on the fly without restart
- **Exclusion Filters** -- `/exclude tag` and `/exclude msg` to hide unwanted lines (LogRabbit-style "None of")
- **Inline Filter Editing** -- `/filter edit` loads current filters into the input line for editing
- **App Tracking** -- automatic PID tracking with re-resolve on app restart; last used filters are auto-restored on reconnect
- **5 Output Presets** -- compact, threadtime, verbose, minimal, json
- **Auto-connect** -- single device is selected automatically; multiple devices show a numbered list
- **Smart Tab-completion** -- `/app` shows package history and current foreground app; `/filter` shows presets associated with the current app
- **Traffic Inspection** -- HTTP/HTTPS proxy via mitmproxy/mitmdump
- **Mock Rules** -- response overrides via YAML config with hot reload
- **Keyboard shortcuts** -- PageUp/Down scroll, Ctrl+C exit, Ctrl+L clear, Tab completion

## Requirements

- [Rust](https://rustup.rs/) 1.70+
- [ADB](https://developer.android.com/tools/adb) (Android Debug Bridge)
- [mitmproxy](https://mitmproxy.org/) (optional, for traffic inspection)

## Installation

### 1. Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2. Build from source

```bash
git clone https://github.com/thothlab/logux.git
cd logux
cargo build --release
```

Binary: `./target/release/logux`

### System-wide install

```bash
cargo install --path .
```

After this, `logux` is available from any directory.

### Update

```bash
cd logux && git pull && cargo build --release && cargo install --path .
```

## Quick Start

```bash
# Launch
logux

# Inside the TUI:
/devices              # List connected devices
/app com.example.app  # Filter by app (auto PID tracking)
/level W              # Show WARN and above only
/grep error           # Text search with highlighting
/format json          # Switch output to JSON
/stop                 # Stop the log stream
```

## Interface

```
┌──────────────────────────────────────────────────────┐
│ 04-13 12:34:56  D  MyTag          Short message      │ ← logs with columns
│ 04-13 12:34:57  W  NetworkManager This is a long     │
│                                    message that wraps │ ← wrap inside column
│ ...                                                   │
├──────────────────────────────────────────────────────┤
│  device_name   com.pkg   STREAMING         120 lines │ ← status bar
├──────────────────────────────────────────────────────┤
│ logux > /app mts_                                     │ ← input (always visible)
└──────────────────────────────────────────────────────┘
```

## Commands

### General

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/exit` (alias: `/quit`, `/q`) | Exit |
| `/clear` | Clear screen |

### ADB

| Command | Description |
|---------|-------------|
| `/devices` | List devices |
| `/connect <ip:port>` | Connect via TCP |
| `/disconnect` | Disconnect |

### Logs & Filtering

| Command | Description |
|---------|-------------|
| `/app <package>` | Filter by app (smart PID tracking) |
| `/pid <pid>` | Filter by PID |
| `/tag <tag>` | Add tag filter (`-tag` remove, `reset` clear) |
| `/level <V\|D\|I\|W\|E\|F>` | Minimum log level (`reset` to clear) |
| `/grep <text>` | Text search in tag + message, case-insensitive (`reset` to clear) |
| `/msg <text>` | Text search in message only (`-text` remove, `reset` clear) |
| `/regex <pattern>` | Regex search (`reset` to clear) |
| `/exclude tag <name>` | Exclude tag from output |
| `/exclude msg <text>` | Exclude lines containing text |
| `/exclude show` | Show exclusion filters |
| `/exclude reset` | Clear all exclusions |
| `/exclude remove <value>` | Remove one exclusion |
| `/filter` | Edit filters inline (= `/filter edit`) |
| `/filter show` | Show active filters |
| `/filter set <expr>` | Set filters in one line |
| `/filter reset` | Clear all filters |
| `/filter <preset>` | Load a saved preset |

#### How Filters Work

**Retroactive filtering (Android Studio style):** changing any filter immediately re-filters the entire log buffer -- not just new lines. All previously received entries are kept in memory and re-evaluated against the updated filters. This works exactly like the logcat panel in Android Studio.

**All filters use `contains` (partial match), not exact match.**
For example, `tag=anal` matches tags "Analytics", "AnalyticsTracker", "DataAnalysis".
For exact match, use regex with anchors: `/regex ^Analytics$`.

**Inclusion filters** combine with **AND** (all conditions must match):
```
/app ru.lewis.dbo    — by app
/tag network         — + tag contains "network"
/level W             — + level >= WARN
/grep timeout        — + text contains "timeout"
```
Result: show only lines where app=ru.lewis.dbo **AND** tag contains "network" **AND** level >= W **AND** text contains "timeout".

**OR within same filter type**: multiple tags (`/tag A`, then `/tag B`) work as OR -- a line passes if tag contains A **OR** B.

**Exclusion filters** (LogRabbit-style "None of the following"):
```
/exclude tag System.out       — hide tag containing "System.out"
/exclude tag CatalogParser    — hide another
/exclude msg "[socket]:check" — hide lines with text
```

#### Inline Filter Editing

`/filter` or `/filter edit` loads current filters into the input line:
```
logux > /filter set app=ru.lewis.dbo tag=network level=W !tag=System.out,Instana
```
Edit and press Enter. Format: space-separated `key=value` pairs.

| Key | Description |
|-----|-------------|
| `app=X` | Filter by app |
| `tag=A,B` | Tags (OR via comma) |
| `level=W` | Minimum level |
| `grep=text` | Text search (tag + message) |
| `msg=text` | Text search (message only, repeat for OR) |
| `regex=pattern` | Regex |
| `!tag=X,Y` | Exclude tags |
| `!msg=text` | Exclude by text |

**Auto-save**: every `/filter set` is saved automatically. Previously used combinations are shown as suggestions on next `/filter`.

**Per-app filter memory**: filters are automatically saved per app package. When you reconnect to the same app with `/app <package>`, your last used filters (tags, level, grep, excludes) are restored automatically.

### Format

| Command | Description |
|---------|-------------|
| `/format <preset>` | compact / threadtime / verbose / minimal / json |
| `/fields +field -field` | Toggle fields: timestamp, level, tag, pid, tid |

### Control

| Command | Description |
|---------|-------------|
| `/stop` | Stop the log stream completely |
| `/pause` | Toggle pause (logs captured but hidden) |
| `/resume` | Resume after pause |
| `/save <file>` | Save matching logs to file |

### Presets

| Command | Description |
|---------|-------------|
| `/preset save <name>` | Save current configuration |
| `/preset load <name>` | Load a preset |
| `/preset list` | List presets |
| `/preset delete <name>` | Delete a preset |

### Traffic

| Command | Description |
|---------|-------------|
| `/traffic open` | Start proxy |
| `/traffic close` | Stop proxy |
| `/traffic list` | Show captured requests |
| `/traffic inspect <id>` | Request/response details |
| `/traffic filter <expr>` | Filter: host=, path=, method=, status= |
| `/traffic clear` | Clear captured traffic |

### Mock Rules

| Command | Description |
|---------|-------------|
| `/mock load <file.yaml>` | Load rules |
| `/mock list` | List rules |
| `/mock enable <id>` | Enable a rule |
| `/mock disable <id>` | Disable a rule |
| `/mock reload` | Reload rules from file |

### Keyboard Shortcuts & Scrolling

| Key | Action |
|-----|--------|
| `Mouse wheel` | Scroll logs 1 line at a time (requires `/mouse on`) |
| `Shift+Up` / `Shift+Down` | Scroll logs 1 line at a time |
| `PageUp` / `PageDown` | Scroll logs 10 lines at a time |
| `Tab` | Auto-complete |
| `Up` / `Down` | Command history / suggestion navigation |
| `Shift+Enter` / `Alt+Enter` / `Ctrl+J` | Insert newline in input |
| `Ctrl+C` | Exit |
| `Ctrl+L` | Clear logs |
| `Ctrl+U` | Clear input line |
| `Ctrl+W` | Delete word backward |
| `Ctrl+A` / `Ctrl+E` | Beginning / end of line |
| `Esc` | Dismiss suggestions |

**Text selection and copy:** works out of the box (mouse capture is off by default). To enable wheel-scroll use `/mouse on` — in that mode hold Option/Alt while dragging to select text (on macOS).

Scrolling up auto-pauses the stream. Scrolling back down to the bottom resumes it. Status bar shows `SCROLL +N` with a `PageDown to resume` hint.

## YAML Mock Rules Example

```yaml
rules:
  - id: user_profile_mock
    enabled: true
    priority: 10
    match:
      method: GET
      path: /api/v1/profile
      query:
        userId: "123"
    response:
      type: file
      file: mocks/profile_123.json
      status: 200

  - id: force_error
    enabled: false
    match:
      path: /api/v1/payment
    response:
      type: error
      status: 500
```

## Architecture

```
src/
 ├── main.rs              -- entry point (tokio async runtime)
 ├── adb/mod.rs           -- device management, logcat streaming
 ├── cli/
 │   ├── tui.rs            -- TUI: ratatui, event loop, columnar rendering
 │   ├── commands.rs       -- command handlers (buffered output)
 │   └── completer.rs      -- tab completion with package/preset history
 ├── logs/
 │   ├── parser.rs         -- logcat parser (threadtime/brief)
 │   ├── filters.rs        -- composable filters
 │   └── formatter.rs      -- field configuration, presets
 ├── traffic/mod.rs        -- proxy adapter
 ├── mock/mod.rs           -- YAML rules engine
 └── config/mod.rs         -- presets + app/filter history
```

## Versions

| Tag | Language | Description |
|-----|----------|-------------|
| `v2.1.0` | Rust | TUI with columnar layout, ratatui |
| `v2.0.0` | Rust | First Rust version (rustyline REPL) |
| `v1.0.0-python` | Python | Previous version (prompt_toolkit + rich + mitmproxy) |

```bash
# Clone the Python version
git clone --branch v1.0.0-python https://github.com/thothlab/logux.git
```

## License

MIT
