"""Log filter engine — composable filters with inclusion/exclusion + edit-string round-trip."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from datetime import datetime

from .parser import LogEntry, LogLevel


@dataclass
class FilterState:
    """Mutable filter configuration. All fields can be changed at runtime.

    Inclusion filters combine via AND across types; within a type (tags, msgs)
    they combine via OR. Exclusion filters hide lines that match any of them
    (LogRabbit-style "None of the following").
    """

    # Inclusion
    package: str = ""
    pids: set[int] = field(default_factory=set)
    tags: set[str] = field(default_factory=set)
    min_level: LogLevel = LogLevel.VERBOSE
    text: str = ""  # /grep — tag + message substring
    msgs: set[str] = field(default_factory=set)  # /msg — message-only OR list
    regex: re.Pattern[str] | None = None
    threads: set[int] = field(default_factory=set)
    time_start: datetime | None = None
    time_end: datetime | None = None

    # Exclusion (/exclude tag|msg …)
    exclude_tags: set[str] = field(default_factory=set)
    exclude_msgs: set[str] = field(default_factory=set)

    # Internal: track if we need PID auto-update when the app restarts.
    _package_tracking: bool = False

    # --- Mutators ----------------------------------------------------------

    def reset(self) -> None:
        self.package = ""
        self.pids.clear()
        self.tags.clear()
        self.min_level = LogLevel.VERBOSE
        self.text = ""
        self.msgs.clear()
        self.regex = None
        self.threads.clear()
        self.time_start = None
        self.time_end = None
        self.exclude_tags.clear()
        self.exclude_msgs.clear()
        self._package_tracking = False

    def set_package(self, package: str, pid: int | None = None) -> None:
        self.package = package
        self.pids.clear()
        if pid:
            self.pids.add(pid)
        self._package_tracking = bool(package)

    def update_pid(self, pid: int) -> None:
        self.pids.clear()
        self.pids.add(pid)

    def add_tag(self, tag: str) -> None:
        self.tags.add(tag)

    def remove_tag(self, tag: str) -> None:
        self.tags.discard(tag)

    def reset_tags(self) -> None:
        self.tags.clear()

    def set_level(self, level: LogLevel) -> None:
        self.min_level = level

    def reset_level(self) -> None:
        self.min_level = LogLevel.VERBOSE

    def set_text(self, text: str) -> None:
        self.text = text

    def reset_text(self) -> None:
        self.text = ""

    def add_msg(self, text: str) -> None:
        self.msgs.add(text)

    def remove_msg(self, text: str) -> None:
        self.msgs.discard(text)

    def reset_msgs(self) -> None:
        self.msgs.clear()

    def set_regex(self, pattern: str) -> None:
        self.regex = re.compile(pattern, re.IGNORECASE)

    def reset_regex(self) -> None:
        self.regex = None

    def set_pid(self, pid: int) -> None:
        self._package_tracking = False
        self.pids.clear()
        self.pids.add(pid)

    def set_threads(self, tids: set[int]) -> None:
        self.threads = tids

    def set_time_range(self, start: datetime | None, end: datetime | None) -> None:
        self.time_start = start
        self.time_end = end

    def add_exclude_tag(self, tag: str) -> None:
        self.exclude_tags.add(tag)

    def add_exclude_msg(self, text: str) -> None:
        self.exclude_msgs.add(text)

    def remove_exclude(self, value: str) -> bool:
        if value in self.exclude_tags:
            self.exclude_tags.discard(value)
            return True
        if value in self.exclude_msgs:
            self.exclude_msgs.discard(value)
            return True
        return False

    def reset_excludes(self) -> None:
        self.exclude_tags.clear()
        self.exclude_msgs.clear()

    # --- Views -------------------------------------------------------------

    @property
    def description(self) -> str:
        parts: list[str] = []
        if self.package:
            parts.append(f"app={self.package}")
        if self.pids:
            parts.append(f"pid={','.join(str(p) for p in self.pids)}")
        if self.tags:
            parts.append(f"tag={','.join(sorted(self.tags))}")
        if self.min_level > LogLevel.VERBOSE:
            parts.append(f"level>={self.min_level.char}")
        if self.text:
            parts.append(f"grep='{self.text}'")
        if self.msgs:
            parts.append("msg=" + "|".join(sorted(self.msgs)))
        if self.regex:
            parts.append(f"regex='{self.regex.pattern}'")
        if self.exclude_tags:
            parts.append("!tag=" + ",".join(sorted(self.exclude_tags)))
        if self.exclude_msgs:
            parts.append("!msg=" + "|".join(sorted(self.exclude_msgs)))
        return " | ".join(parts) if parts else "no filters"

    def to_edit_string(self) -> str:
        """Render current filters as a space-separated key=value string
        suitable for `/filter set …` round-trip editing."""
        parts: list[str] = []
        if self.package:
            parts.append(f"app={self.package}")
        if self.tags:
            parts.append("tag=" + ",".join(sorted(self.tags)))
        if self.min_level > LogLevel.VERBOSE:
            parts.append(f"level={self.min_level.char}")
        if self.text:
            parts.append(f"grep={self.text}")
        for m in sorted(self.msgs):
            parts.append(f"msg={m}")
        if self.regex:
            parts.append(f"regex={self.regex.pattern}")
        if self.exclude_tags:
            parts.append("!tag=" + ",".join(sorted(self.exclude_tags)))
        for m in sorted(self.exclude_msgs):
            parts.append(f"!msg={m}")
        return " ".join(parts)

    def apply_edit_string(self, expr: str) -> None:
        """Replace current filters with those parsed from `/filter set` expression.
        Format: space-separated key=value pairs. Multi-value via comma (tag,
        !tag) or by repeating the key (msg, !msg)."""
        self.tags.clear()
        self.min_level = LogLevel.VERBOSE
        self.text = ""
        self.msgs.clear()
        self.regex = None
        self.exclude_tags.clear()
        self.exclude_msgs.clear()
        new_package: str | None = None

        for tok in expr.split():
            if "=" not in tok:
                continue
            key, value = tok.split("=", 1)
            key = key.strip()
            value = value.strip()
            if not value:
                continue
            if key == "app":
                new_package = value
            elif key == "tag":
                for t in value.split(","):
                    t = t.strip()
                    if t:
                        self.tags.add(t)
            elif key == "level":
                self.min_level = LogLevel.from_char(value[0])
            elif key == "grep":
                self.text = value
            elif key == "msg":
                self.msgs.add(value)
            elif key == "regex":
                try:
                    self.regex = re.compile(value, re.IGNORECASE)
                except re.error:
                    pass
            elif key == "!tag":
                for t in value.split(","):
                    t = t.strip()
                    if t:
                        self.exclude_tags.add(t)
            elif key == "!msg":
                self.exclude_msgs.add(value)

        if new_package is not None:
            self.package = new_package
            self._package_tracking = bool(new_package)


def matches(entry: LogEntry, state: FilterState) -> bool:
    """Check if a log entry passes all active filters."""

    # --- Inclusion (AND across types) ---
    if entry.level < state.min_level:
        return False

    if state.pids and entry.pid not in state.pids:
        return False

    if state.tags:
        if not any(t.lower() in entry.tag.lower() for t in state.tags):
            return False

    if state.text:
        haystack = f"{entry.tag} {entry.message}".lower()
        if state.text.lower() not in haystack:
            return False

    if state.msgs:
        low = entry.message.lower()
        if not any(m.lower() in low for m in state.msgs):
            return False

    if state.regex and not state.regex.search(f"{entry.tag} {entry.message}"):
        return False

    if state.threads and entry.tid not in state.threads:
        return False

    if state.time_start or state.time_end:
        ts = entry.datetime
        if ts:
            if state.time_start and ts < state.time_start:
                return False
            if state.time_end and ts > state.time_end:
                return False

    # --- Exclusion ---
    if state.exclude_tags:
        low_tag = entry.tag.lower()
        if any(ex.lower() in low_tag for ex in state.exclude_tags):
            return False

    if state.exclude_msgs:
        low_msg = entry.message.lower()
        if any(ex.lower() in low_msg for ex in state.exclude_msgs):
            return False

    return True
