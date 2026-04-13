"""Command auto-completer for the interactive shell."""

from __future__ import annotations

from prompt_toolkit.completion import Completer, Completion
from prompt_toolkit.document import Document

from ..logs.parser import LogLevel
from ..logs.formatter import Preset


COMMANDS: dict[str, list[str]] = {
    "/help": [],
    "/exit": [],
    "/clear": [],
    # ADB
    "/devices": [],
    "/connect": ["<ip:port>"],
    "/disconnect": [],
    # Logs
    "/app": ["<package.name>"],
    "/pid": ["<pid>"],
    "/tag": ["<tag>"],
    "/level": [l.name.lower() for l in LogLevel if l != LogLevel.SILENT],
    "/grep": ["<text>"],
    "/regex": ["<pattern>"],
    "/filter": ["reset", "show"],
    # Format
    "/format": [p.value for p in Preset],
    "/fields": ["+timestamp", "-timestamp", "+level", "-level", "+tag", "-tag",
                "+pid", "-pid", "+tid", "-tid", "+message", "-message"],
    # Control
    "/pause": [],
    "/resume": [],
    "/save": ["<file>"],
    # Presets
    "/preset": ["save", "load", "list", "delete"],
    # Traffic
    "/traffic": ["open", "close", "list", "inspect", "filter", "clear"],
    # Mock
    "/mock": ["load", "list", "enable", "disable", "reload"],
}


class LoguxCompleter(Completer):
    def get_completions(self, document: Document, complete_event: object) -> list[Completion]:
        text = document.text_before_cursor.lstrip()

        if not text.startswith("/"):
            return []

        parts = text.split(maxsplit=1)
        cmd = parts[0]

        # Complete command name
        if len(parts) == 1 and not text.endswith(" "):
            for name in COMMANDS:
                if name.startswith(cmd):
                    yield Completion(name, start_position=-len(cmd))
            return

        # Complete subcommand / argument
        if cmd in COMMANDS and COMMANDS[cmd]:
            arg_text = parts[1] if len(parts) > 1 else ""
            for opt in COMMANDS[cmd]:
                if opt.startswith("<"):
                    continue
                if opt.startswith(arg_text):
                    yield Completion(opt, start_position=-len(arg_text))
