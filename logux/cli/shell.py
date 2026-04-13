"""Interactive CLI shell — REPL with async log streaming."""

from __future__ import annotations

import asyncio
import signal
import sys

from prompt_toolkit import PromptSession
from prompt_toolkit.history import FileHistory
from prompt_toolkit.patch_stdout import patch_stdout
from prompt_toolkit.styles import Style
from rich.console import Console
from rich.panel import Panel
from rich.text import Text
from pathlib import Path

from ..adb.client import ADBClient
from ..logs.stream import LogStream
from ..traffic.proxy import TrafficProxy
from ..mock.rules import MockEngine
from .completer import LoguxCompleter
from .commands import dispatch


BANNER = r"""
 ╦  ╔═╗╔═╗╦ ╦═╗ ╦
 ║  ║ ║║ ╦║ ║╔╩╦╝
 ╩═╝╚═╝╚═╝╚═╝╩ ╚═  v1.0
"""

PROMPT_STYLE = Style.from_dict({
    "prompt": "ansicyan bold",
    "": "ansiwhite",
})


class LoguxShell:
    """Main application shell — ties together all modules."""

    def __init__(self) -> None:
        self.console = Console()
        self.adb = ADBClient()
        self.log_stream = LogStream(self.adb, self.console)
        self.traffic = TrafficProxy(self.console)
        self.mock_engine = MockEngine(self.console)
        self._exit_requested = False

        history_dir = Path.home() / ".logux"
        history_dir.mkdir(parents=True, exist_ok=True)
        history_file = history_dir / "history"

        self.session: PromptSession[str] = PromptSession(
            history=FileHistory(str(history_file)),
            completer=LoguxCompleter(),
            style=PROMPT_STYLE,
            complete_while_typing=True,
        )

    def request_exit(self) -> None:
        self._exit_requested = True

    def _build_prompt(self) -> list[tuple[str, str]]:
        parts: list[tuple[str, str]] = []
        parts.append(("class:prompt", "logux"))

        if self.adb.selected_device:
            dev = self.adb.selected_device
            name = dev.model or dev.serial
            parts.append(("", f"@{name}"))

        if self.log_stream.filters.package:
            parts.append(("ansiyellow", f" [{self.log_stream.filters.package}]"))

        if self.log_stream.is_paused:
            parts.append(("ansired", " (paused)"))
        elif self.log_stream.is_running:
            parts.append(("ansigreen", " (streaming)"))

        if self.traffic.is_running:
            parts.append(("ansimagenta", " (proxy)"))

        parts.append(("class:prompt", " > "))
        return parts

    async def run(self) -> None:
        self.console.print(Text(BANNER, style="bold cyan"))
        self.console.print("[dim]Type /help for commands, /exit to quit[/dim]\n")

        # Check ADB availability
        ok, version = self.adb.check_adb()
        if ok:
            self.console.print(f"[green]ADB: {version}[/green]")
        else:
            self.console.print(f"[red]ADB: {version}[/red]")

        # List devices on start
        devices = self.adb.list_devices()
        if devices:
            online = [d for d in devices if d.is_online]
            self.console.print(f"[dim]Devices: {len(online)} online / {len(devices)} total[/dim]")
            if len(online) == 1:
                self.adb.selected_device = online[0]
                self.console.print(f"[green]Auto-selected: {online[0].display_name}[/green]")
        else:
            self.console.print("[yellow]No devices connected[/yellow]")

        self.console.print()

        # PID watcher task
        pid_watcher = asyncio.create_task(self._pid_watcher())

        try:
            while not self._exit_requested:
                try:
                    with patch_stdout():
                        user_input = await asyncio.get_event_loop().run_in_executor(
                            None,
                            lambda: self.session.prompt(self._build_prompt()),
                        )
                except (EOFError, KeyboardInterrupt):
                    break

                user_input = user_input.strip()
                if not user_input:
                    continue

                if user_input.startswith("/"):
                    await dispatch(self, user_input)
                else:
                    # Treat as grep shortcut
                    self.log_stream.filters.set_text(user_input)
                    self.log_stream.formatter.highlight_text = user_input
                    self.console.print(f"[green]Quick filter: '{user_input}'[/green]")

        finally:
            pid_watcher.cancel()
            await self.log_stream.stop()
            self.traffic.stop()
            self.console.print("\n[dim]Bye![/dim]")

    async def _pid_watcher(self) -> None:
        """Periodically re-resolve PID when tracking a package."""
        while True:
            await asyncio.sleep(3)
            try:
                filters = self.log_stream.filters
                if filters._package_tracking and filters.package:
                    new_pid = self.adb.get_pid(filters.package)
                    if new_pid and new_pid not in filters.pids:
                        filters.update_pid(new_pid)
                        self.console.print(
                            f"\n[yellow]App restarted — new PID: {new_pid}[/yellow]"
                        )

                # Check hot reload for mock rules
                if self.mock_engine.rules:
                    self.mock_engine.check_hot_reload()
            except asyncio.CancelledError:
                return
            except Exception:
                pass
