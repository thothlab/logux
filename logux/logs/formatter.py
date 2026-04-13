"""Log formatter — colored output with configurable fields and presets."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
import json

from rich.text import Text

from .parser import LogEntry, LogLevel


class Preset(Enum):
    COMPACT = "compact"
    THREADTIME = "threadtime"
    VERBOSE = "verbose"
    MINIMAL = "minimal"
    JSON = "json"


# Color scheme for log levels
LEVEL_STYLES: dict[LogLevel, str] = {
    LogLevel.VERBOSE: "dim white",
    LogLevel.DEBUG: "blue",
    LogLevel.INFO: "green",
    LogLevel.WARN: "yellow",
    LogLevel.ERROR: "bold red",
    LogLevel.FATAL: "bold white on red",
    LogLevel.SILENT: "dim",
}

TAG_COLORS = [
    "cyan", "magenta", "bright_blue", "bright_green",
    "bright_yellow", "bright_magenta", "bright_cyan",
    "dark_orange", "purple", "deep_sky_blue1",
]


def _tag_color(tag: str) -> str:
    idx = hash(tag) % len(TAG_COLORS)
    return TAG_COLORS[idx]


@dataclass
class FormatConfig:
    """Which fields to display."""
    timestamp: bool = True
    level: bool = True
    tag: bool = True
    pid: bool = True
    tid: bool = False
    message: bool = True
    preset: Preset = Preset.COMPACT

    def apply_preset(self, preset: Preset) -> None:
        self.preset = preset
        if preset == Preset.COMPACT:
            self.timestamp = True
            self.level = True
            self.tag = True
            self.pid = False
            self.tid = False
            self.message = True
        elif preset == Preset.THREADTIME:
            self.timestamp = True
            self.level = True
            self.tag = True
            self.pid = True
            self.tid = True
            self.message = True
        elif preset == Preset.VERBOSE:
            self.timestamp = True
            self.level = True
            self.tag = True
            self.pid = True
            self.tid = True
            self.message = True
        elif preset == Preset.MINIMAL:
            self.timestamp = False
            self.level = True
            self.tag = True
            self.pid = False
            self.tid = False
            self.message = True
        elif preset == Preset.JSON:
            pass  # JSON mode handled separately

    def toggle_field(self, name: str, enabled: bool) -> bool:
        if hasattr(self, name) and isinstance(getattr(self, name), bool):
            setattr(self, name, enabled)
            return True
        return False


# Highlight patterns for special content
_STACKTRACE_MARKERS = ("at ", "Caused by:", "java.", "kotlin.", "android.")


@dataclass
class LogFormatter:
    config: FormatConfig = field(default_factory=FormatConfig)
    highlight_text: str = ""

    def format_entry(self, entry: LogEntry) -> Text:
        if self.config.preset == Preset.JSON:
            return self._format_json(entry)
        return self._format_rich(entry)

    def _format_rich(self, entry: LogEntry) -> Text:
        text = Text()
        level_style = LEVEL_STYLES.get(entry.level, "")

        # Timestamp
        if self.config.timestamp and entry.timestamp:
            text.append(entry.timestamp, style="dim cyan")
            text.append(" ")

        # Level
        if self.config.level:
            text.append(f" {entry.level.char} ", style=f"bold {level_style}")
            text.append(" ")

        # PID/TID
        if self.config.pid and entry.pid:
            text.append(f"{entry.pid:>5}", style="dim")
            if self.config.tid and entry.tid:
                text.append(f"/{entry.tid:<5}", style="dim")
            text.append(" ")
        elif self.config.tid and entry.tid:
            text.append(f"{entry.tid:>5}", style="dim")
            text.append(" ")

        # Tag
        if self.config.tag and entry.tag:
            tag_style = _tag_color(entry.tag)
            tag_display = entry.tag[:24].ljust(24)
            text.append(tag_display, style=tag_style)
            text.append(" ")

        # Message
        if self.config.message:
            msg = entry.message
            is_stacktrace = any(msg.lstrip().startswith(m) for m in _STACKTRACE_MARKERS)

            if is_stacktrace:
                text.append(msg, style="dim red italic")
            elif self.highlight_text and self.highlight_text.lower() in msg.lower():
                self._append_highlighted(text, msg, self.highlight_text, level_style)
            else:
                text.append(msg, style=level_style)

        return text

    def _append_highlighted(self, text: Text, msg: str, needle: str, base_style: str) -> None:
        lower_msg = msg.lower()
        lower_needle = needle.lower()
        pos = 0
        while True:
            idx = lower_msg.find(lower_needle, pos)
            if idx == -1:
                text.append(msg[pos:], style=base_style)
                break
            text.append(msg[pos:idx], style=base_style)
            text.append(msg[idx:idx + len(needle)], style="bold black on yellow")
            pos = idx + len(needle)

    def _format_json(self, entry: LogEntry) -> Text:
        data = {
            "timestamp": entry.timestamp,
            "level": entry.level.char,
            "pid": entry.pid,
            "tid": entry.tid,
            "tag": entry.tag,
            "message": entry.message,
        }
        return Text(json.dumps(data, ensure_ascii=False))
