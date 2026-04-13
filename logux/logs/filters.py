"""Log filter engine — composable filters that can be changed on the fly."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from datetime import datetime

from .parser import LogEntry, LogLevel


@dataclass
class FilterState:
    """Mutable filter configuration. All fields can be changed at runtime."""

    package: str = ""
    pids: set[int] = field(default_factory=set)
    tags: set[str] = field(default_factory=set)
    min_level: LogLevel = LogLevel.VERBOSE
    text: str = ""
    regex: re.Pattern[str] | None = None
    threads: set[int] = field(default_factory=set)
    time_start: datetime | None = None
    time_end: datetime | None = None

    # Internal: track if we need PID auto-update
    _package_tracking: bool = False

    def reset(self) -> None:
        self.package = ""
        self.pids.clear()
        self.tags.clear()
        self.min_level = LogLevel.VERBOSE
        self.text = ""
        self.regex = None
        self.threads.clear()
        self.time_start = None
        self.time_end = None
        self._package_tracking = False

    def set_package(self, package: str, pid: int | None = None) -> None:
        self.package = package
        self.pids.clear()
        if pid:
            self.pids.add(pid)
        self._package_tracking = bool(package)

    def update_pid(self, pid: int) -> None:
        """Called when app restarts and gets a new PID."""
        self.pids.clear()
        self.pids.add(pid)

    def add_tag(self, tag: str) -> None:
        self.tags.add(tag)

    def remove_tag(self, tag: str) -> None:
        self.tags.discard(tag)

    def set_level(self, level: LogLevel) -> None:
        self.min_level = level

    def set_text(self, text: str) -> None:
        self.text = text

    def set_regex(self, pattern: str) -> None:
        self.regex = re.compile(pattern, re.IGNORECASE)

    def set_pid(self, pid: int) -> None:
        self._package_tracking = False
        self.pids.clear()
        self.pids.add(pid)

    def set_threads(self, tids: set[int]) -> None:
        self.threads = tids

    def set_time_range(self, start: datetime | None, end: datetime | None) -> None:
        self.time_start = start
        self.time_end = end

    @property
    def description(self) -> str:
        parts: list[str] = []
        if self.package:
            parts.append(f"app={self.package}")
        if self.pids:
            parts.append(f"pid={','.join(str(p) for p in self.pids)}")
        if self.tags:
            parts.append(f"tag={','.join(self.tags)}")
        if self.min_level > LogLevel.VERBOSE:
            parts.append(f"level>={self.min_level.char}")
        if self.text:
            parts.append(f"text='{self.text}'")
        if self.regex:
            parts.append(f"regex='{self.regex.pattern}'")
        if self.threads:
            parts.append(f"thread={','.join(str(t) for t in self.threads)}")
        return " | ".join(parts) if parts else "no filters"


def matches(entry: LogEntry, state: FilterState) -> bool:
    """Check if a log entry passes all active filters."""

    # Level filter
    if entry.level < state.min_level:
        return False

    # PID filter
    if state.pids and entry.pid not in state.pids:
        return False

    # Tag filter
    if state.tags:
        if not any(t.lower() in entry.tag.lower() for t in state.tags):
            return False

    # Text filter (case-insensitive substring)
    if state.text:
        haystack = f"{entry.tag} {entry.message}".lower()
        if state.text.lower() not in haystack:
            return False

    # Regex filter
    if state.regex:
        haystack = f"{entry.tag} {entry.message}"
        if not state.regex.search(haystack):
            return False

    # Thread filter
    if state.threads and entry.tid not in state.threads:
        return False

    # Time range filter
    if state.time_start or state.time_end:
        ts = entry.datetime
        if ts:
            if state.time_start and ts < state.time_start:
                return False
            if state.time_end and ts > state.time_end:
                return False

    return True
