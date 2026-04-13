"""Log stream — async pipeline: logcat → parser → filter → formatter → output."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

from rich.console import Console

from .parser import parse_logcat_line
from .filters import FilterState, matches
from .formatter import LogFormatter

if TYPE_CHECKING:
    from ..adb.client import ADBClient


class LogStream:
    """Manages the async log pipeline from ADB to terminal output."""

    def __init__(self, adb: ADBClient, console: Console) -> None:
        self.adb = adb
        self.console = console
        self.filters = FilterState()
        self.formatter = LogFormatter()
        self._process: asyncio.subprocess.Process | None = None
        self._task: asyncio.Task[None] | None = None
        self._paused = False
        self._running = False
        self._lines_count = 0
        self._save_file: str | None = None
        self._save_handle = None

    @property
    def is_running(self) -> bool:
        return self._running

    @property
    def is_paused(self) -> bool:
        return self._paused

    @property
    def lines_count(self) -> int:
        return self._lines_count

    async def start(self, clear: bool = False) -> None:
        if self._running:
            await self.stop()

        self._process = await self.adb.stream_logcat(clear_first=clear)
        self._running = True
        self._paused = False
        self._lines_count = 0
        self._task = asyncio.create_task(self._read_loop())

    async def stop(self) -> None:
        self._running = False
        if self._process:
            try:
                self._process.terminate()
                await asyncio.wait_for(self._process.wait(), timeout=2)
            except (ProcessLookupError, asyncio.TimeoutError):
                try:
                    self._process.kill()
                except ProcessLookupError:
                    pass
            self._process = None
        if self._task and not self._task.done():
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
            self._task = None
        self._close_save()

    def pause(self) -> None:
        self._paused = True

    def resume(self) -> None:
        self._paused = False

    def start_save(self, filepath: str) -> None:
        self._close_save()
        self._save_file = filepath
        self._save_handle = open(filepath, "a", encoding="utf-8")

    def _close_save(self) -> None:
        if self._save_handle:
            self._save_handle.close()
            self._save_handle = None
        self._save_file = None

    async def _read_loop(self) -> None:
        if not self._process or not self._process.stdout:
            return

        try:
            async for raw_line in self._process.stdout:
                if not self._running:
                    break

                line = raw_line.decode("utf-8", errors="replace")
                entry = parse_logcat_line(line)
                if entry is None:
                    continue

                if not matches(entry, self.filters):
                    continue

                if self._paused:
                    continue

                self._lines_count += 1
                formatted = self.formatter.format_entry(entry)
                self.console.print(formatted, highlight=False)

                if self._save_handle:
                    self._save_handle.write(entry.raw + "\n")
                    self._save_handle.flush()

        except asyncio.CancelledError:
            pass
        except Exception as exc:
            self.console.print(f"[red]Stream error: {exc}[/red]")
        finally:
            self._running = False
            self._close_save()

    async def restart_with_new_pid(self, package: str) -> bool:
        """Re-resolve PID for a package and update filter."""
        pid = self.adb.get_pid(package)
        if pid:
            self.filters.update_pid(pid)
            return True
        return False
