# logux — Python edition (v2.0)

[![Python](https://img.shields.io/badge/python-3.10%2B-blue)](https://www.python.org/) [![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Android **Logs & Traffic** CLI — real-time `adb logcat` viewer with smart filtering, presets, traffic inspection via mitmproxy, and mock rules.

> The active implementation lives on the `main` branch and is written in **Rust** (v2.1+). This branch (`python-2.0`) is the Python edition brought up to feature-parity with Rust v2.1.

## Features

- **Resilient stream** — UTF-8-lossy decoding + auto-reconnect with backoff (5 attempts: 0.5 / 1 / 2 / 5 / 10 s); survives garbled bytes and `adb` disconnects.
- **`/reconnect`** — hard-reset the adb server (`kill-server` + `start-server`), re-list devices, refresh the tracked PID, resume streaming.
- **Filter engine** — inclusion (`/tag`, `/level`, `/grep`, `/msg`, `/regex`, `/pid`, `/app`) + exclusion (`/exclude tag|msg`); round-trip editable via `/filter edit` → `/filter set`.
- **Per-app filter memory** — filters auto-save under the current package and restore when you switch back with `/app`.
- **Auto-saved filter expressions** — every `/filter set` is remembered for tab-completion.
- **`/copy [N]`** — copy the last N matching messages to the system clipboard (via `pyperclip`).
- **Stream control** — `/pause` (toggle), `/resume`, `/stop`, `/save <file>` / `/save off`.
- **Traffic inspection** — `/traffic open|close|list|inspect|filter|clear` via mitmproxy.
- **Mock rules** — `/mock load|list|enable|disable|reload` with hot-reload from YAML.
- **Presets** — named `/preset save|load|list|delete`, plus `/forget` to wipe all memory.

## Install

```bash
git clone https://github.com/thothlab/logux.git
cd logux
git checkout python-2.0
python3 -m pip install -e .
logux
```

## Command reference

Run `/help` inside the shell. Highlights:

| Command | Description |
| --- | --- |
| `/devices [N]` | List devices; pick by number |
| `/connect <ip:port>` | TCP connect |
| `/reconnect` | Hard-reset adb server + restart stream |
| `/app <pkg>` | Track app (auto PID + restore saved filters) |
| `/tag <t>` / `/tag -<t>` / `/tag reset` | Tag inclusion |
| `/level <V\|D\|I\|W\|E\|F>` / `/level reset` | Minimum level |
| `/grep <text>` / `/grep reset` | Tag+msg substring |
| `/msg <text>` / `/msg reset` | Message-only OR filter |
| `/regex <pat>` / `/regex reset` | Regex over tag+msg |
| `/exclude tag\|msg <v>` / `show` / `reset` / `remove <v>` | Hide matching |
| `/filter show\|reset\|edit\|set <expr>` | Bulk filter ops |
| `/pause`, `/resume`, `/stop` | Stream control |
| `/save <file>` / `/save off` | Persist matching lines |
| `/copy [N]` | Clipboard copy (default 100) |
| `/preset save\|load\|list\|delete <name>` | Named presets |
| `/forget` | Wipe auto-saved filters + per-app memory + history |

## Requirements

- Python ≥ 3.10
- `adb` in `PATH`
- macOS / Linux / Windows (clipboard via `pyperclip` requires a desktop session)

## Related

- **Rust edition** (recommended for production use): checkout `main`, tag `v2.1.0`, prebuilt macOS binaries on the [GitHub Releases page](https://github.com/thothlab/logux/releases).

## License

MIT — see [LICENSE](LICENSE).
