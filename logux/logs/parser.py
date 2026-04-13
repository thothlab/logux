"""Logcat output parser — converts raw lines into structured LogEntry objects."""

from __future__ import annotations

import re
from dataclasses import dataclass
from datetime import datetime
from enum import IntEnum


class LogLevel(IntEnum):
    VERBOSE = 0
    DEBUG = 1
    INFO = 2
    WARN = 3
    ERROR = 4
    FATAL = 5
    SILENT = 6

    @classmethod
    def from_char(cls, char: str) -> LogLevel:
        mapping = {
            "V": cls.VERBOSE,
            "D": cls.DEBUG,
            "I": cls.INFO,
            "W": cls.WARN,
            "E": cls.ERROR,
            "F": cls.FATAL,
            "S": cls.SILENT,
        }
        return mapping.get(char.upper(), cls.VERBOSE)

    @property
    def char(self) -> str:
        return "VDIWEFS"[self.value]


@dataclass(slots=True)
class LogEntry:
    timestamp: str
    pid: int
    tid: int
    level: LogLevel
    tag: str
    message: str
    raw: str

    @property
    def datetime(self) -> datetime | None:
        try:
            return datetime.strptime(self.timestamp, "%m-%d %H:%M:%S.%f")
        except ValueError:
            return None


# threadtime format: "MM-DD HH:MM:SS.mmm  PID  TID LEVEL TAG: MESSAGE"
_THREADTIME_RE = re.compile(
    r"^(\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3})\s+"
    r"(\d+)\s+(\d+)\s+"
    r"([VDIWEFS])\s+"
    r"(.+?)\s*:\s+"
    r"(.*)$"
)

# brief format: "LEVEL/TAG(PID): MESSAGE"
_BRIEF_RE = re.compile(
    r"^([VDIWEFS])/(.+?)\(\s*(\d+)\):\s+(.*)$"
)


def parse_logcat_line(line: str) -> LogEntry | None:
    """Parse a single logcat line. Returns None if the line cannot be parsed."""
    line = line.rstrip()
    if not line:
        return None

    # Skip logcat header lines like "--------- beginning of main"
    if line.startswith("---------"):
        return None

    # Try threadtime format first (default for our streaming)
    m = _THREADTIME_RE.match(line)
    if m:
        return LogEntry(
            timestamp=m.group(1),
            pid=int(m.group(2)),
            tid=int(m.group(3)),
            level=LogLevel.from_char(m.group(4)),
            tag=m.group(5).strip(),
            message=m.group(6),
            raw=line,
        )

    # Try brief format
    m = _BRIEF_RE.match(line)
    if m:
        return LogEntry(
            timestamp="",
            pid=int(m.group(3)),
            tid=0,
            level=LogLevel.from_char(m.group(1)),
            tag=m.group(2).strip(),
            message=m.group(4),
            raw=line,
        )

    # Unparseable — treat as continuation of previous message
    return LogEntry(
        timestamp="",
        pid=0,
        tid=0,
        level=LogLevel.VERBOSE,
        tag="",
        message=line,
        raw=line,
    )
