# logux

[![ru](https://img.shields.io/badge/lang-Русский-green)](README.md)

**Android Logs & Traffic CLI** -- an interactive tool for Android developers: real-time log viewing, filtering, traffic inspection, and network response mocking.

---

## Features

- **ADB Logs** -- reads `adb logcat` with colored, formatted output
- **Smart Filtering** -- by package, tag, level, PID, regex, text -- all changeable on the fly without restart
- **App Tracking** -- automatic PID tracking with re-resolve on app restart
- **5 Output Presets** -- compact, threadtime, verbose, minimal, json
- **Traffic Inspection** -- HTTP/HTTPS proxy via mitmproxy/mitmdump
- **Mock Rules** -- response overrides via YAML config with hot reload
- **Interactive CLI** -- REPL with tab completion, command history, and hints

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

Binary: `./target/release/logux` (3.5 MB, no external dependencies)

### System-wide install

```bash
cargo install --path .
```

After this, `logux` is available from any directory.

## Quick Start

```bash
# Launch
logux

# Inside the REPL:
/devices              # List connected devices
/app com.example.app  # Filter by app (auto PID tracking)
/level W              # Show WARN and above only
/grep error           # Text search with highlighting
/format json          # Switch output to JSON
```

## Commands

### General

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/exit` | Exit |
| `/clear` | Clear screen |

### ADB

| Command | Description |
|---------|-------------|
| `/devices` | List devices |
| `/connect <ip:port>` | Connect via TCP |
| `/disconnect` | Disconnect |

### Logs

| Command | Description |
|---------|-------------|
| `/app <package>` | Filter by app (smart PID tracking) |
| `/pid <pid>` | Filter by PID |
| `/tag <tag>` | Filter by tag |
| `/level <V\|D\|I\|W\|E\|F>` | Minimum log level |
| `/grep <text>` | Text search (case-insensitive) |
| `/regex <pattern>` | Regex search |
| `/filter reset` | Clear all filters |
| `/filter show` | Show active filters |

### Format

| Command | Description |
|---------|-------------|
| `/format <preset>` | compact / threadtime / verbose / minimal / json |
| `/fields +field -field` | Toggle fields: timestamp, level, tag, pid, tid |

### Control

| Command | Description |
|---------|-------------|
| `/pause` | Pause output |
| `/resume` | Resume output |
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
 │   ├── shell.rs          -- interactive REPL
 │   ├── commands.rs       -- command handlers
 │   └── completer.rs      -- tab completion
 ├── logs/
 │   ├── parser.rs         -- logcat parser (threadtime/brief)
 │   ├── filters.rs        -- composable filters
 │   └── formatter.rs      -- colored output, presets
 ├── traffic/mod.rs        -- proxy adapter
 ├── mock/mod.rs           -- YAML rules engine
 └── config/mod.rs         -- preset system
```

## Versions

| Tag | Language | Description |
|-----|----------|-------------|
| `v2.0.0` | Rust | Current version -- single 3.5 MB binary |
| `v1.0.0-python` | Python | Previous version (prompt_toolkit + rich + mitmproxy) |

```bash
# Clone the Python version
git clone --branch v1.0.0-python https://github.com/thothlab/logux.git
```

## License

MIT
