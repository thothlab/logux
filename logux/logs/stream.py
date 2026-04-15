"""Log stream — async pipeline: logcat → parser → filter → formatter → output.

Includes:
- UTF-8 lossy decoding (errors="replace") so garbled bytes don't kill the stream
- Auto-reconnect with backoff on adb logcat exit / I/O error
- Ring buffer of recent messages for /copy
- Status callback so the shell can surface reconnect/retry events
"""

from __future__ import annotations

import asyncio
from collections import deque
from enum import Enum
from typing import TYPE_CHECKING, Callable

from rich.console import Console

from .parser import parse_logcat_line
from .filters import FilterState, matches
from .formatter import LogFormatter

if TYPE_CHECKING:
    from ..adb.client import ADBClient


MAX_COPY_BUFFER = 10_000
MAX_RECONNECT_ATTEMPTS = 5
RECONNECT_BACKOFF_SEC = [0.5, 1.0, 2.0, 5.0, 10.0]


class StreamStatus(Enum):
    STOPPED_BY_USER = "stopped_by_user"
    LOGCAT_EXITED = "logcat_exited"
    IO_ERROR = "io_error"
    RECONNECTING = "reconnecting"
    RECONNECTED = "reconnected"
    GAVE_UP = "gave_up"


class LogStream:
    """Manages the async log pipeline from ADB to terminal output."""

    def __init__(
        self,
        adb: ADBClient,
        console: Console,
        status_callback: Callable[[StreamStatus, str], None] | None = None,
    ) -> None:
        self.adb = adb
        self.console = console
        self.filters = FilterState()
        self.formatter = LogFormatter()
        self._process: asyncio.subprocess.Process | None = None
        self._task: asyncio.Task[None] | None = None
        self._paused = False
        self._running = False
        self._stopped_by_user = False
        self._lines_count = 0
        self._save_file: str | None = None
        self._save_handle = None
        self._recent_messages: deque[str] = deque(maxlen=MAX_COPY_BUFFER)
        self._reconnect_attempts = 0
        self._status_cb = status_callback

    # --- Properties --------------------------------------------------------

    @property
    def is_running(self) -> bool:
        return self._running

    @property
    def is_paused(self) -> bool:
        return self._paused

    @property
    def lines_count(self) -> int:
        return self._lines_count

    @property
    def save_path(self) -> str | None:
        return self._save_file

    @property
    def recent_messages(self) -> list[str]:
        return list(self._recent_messages)

    # --- Lifecycle ---------------------------------------------------------

    async def start(self, clear: bool = False) -> None:
        """Start (or restart) the log stream with auto-reconnect enabled."""
        if self._running:
            await self.stop()

        self._stopped_by_user = False
        self._reconnect_attempts = 0
        self._lines_count = 0
        self._running = True
        self._paused = False
        self._task = asyncio.create_task(self._supervisor_loop(clear_first=clear))

    async def stop(self) -> None:
        """Stop the stream and cancel auto-reconnect."""
        self._stopped_by_user = True
        self._running = False
        await self._kill_process()
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

    def toggle_pause(self) -> bool:
        self._paused = not self._paused
        return self._paused

    # --- /save -------------------------------------------------------------

    def start_save(self, filepath: str) -> None:
        self._close_save()
        self._save_file = filepath
        self._save_handle = open(filepath, "a", encoding="utf-8")

    def stop_save(self) -> None:
        self._close_save()

    def _close_save(self) -> None:
        if self._save_handle:
            try:
                self._save_handle.close()
            except Exception:
                pass
            self._save_handle = None
        self._save_file = None

    # --- Supervisor (auto-reconnect) --------------------------------------

    async def _supervisor_loop(self, clear_first: bool) -> None:
        """Outer loop: spawns logcat, reads, and restarts with backoff on failure."""
        try:
            while self._running and not self._stopped_by_user:
                try:
                    self._process = await self.adb.stream_logcat(clear_first=clear_first)
                except Exception as e:
                    self._emit(StreamStatus.IO_ERROR, f"failed to spawn adb logcat: {e}")
                    if not await self._backoff_or_give_up(str(e)):
                        break
                    continue

                clear_first = False  # only first iteration clears
                exit_reason = await self._read_until_exit()

                if self._stopped_by_user:
                    return

                if exit_reason == StreamStatus.LOGCAT_EXITED:
                    self._emit(
                        StreamStatus.LOGCAT_EXITED,
                        "adb logcat exited (device may have disconnected)",
                    )
                elif exit_reason == StreamStatus.IO_ERROR:
                    pass  # already emitted by _read_until_exit
                else:
                    return

                if not await self._backoff_or_give_up(exit_reason.value):
                    break
        finally:
            self._running = False
            self._close_save()

    async def _backoff_or_give_up(self, reason: str) -> bool:
        if self._reconnect_attempts >= MAX_RECONNECT_ATTEMPTS:
            self._emit(
                StreamStatus.GAVE_UP,
                f"auto-reconnect gave up after {MAX_RECONNECT_ATTEMPTS} attempts "
                f"({reason}). Run /reconnect to reset `adb` and retry.",
            )
            return False

        idx = min(self._reconnect_attempts, len(RECONNECT_BACKOFF_SEC) - 1)
        delay = RECONNECT_BACKOFF_SEC[idx]
        self._reconnect_attempts += 1
        self._emit(
            StreamStatus.RECONNECTING,
            f"reconnecting in {delay:.1f}s "
            f"(attempt {self._reconnect_attempts}/{MAX_RECONNECT_ATTEMPTS}, {reason})",
        )
        try:
            await asyncio.sleep(delay)
        except asyncio.CancelledError:
            return False
        return self._running and not self._stopped_by_user

    async def _read_until_exit(self) -> StreamStatus:
        if not self._process or not self._process.stdout:
            return StreamStatus.IO_ERROR

        try:
            async for raw_line in self._process.stdout:
                if not self._running or self._stopped_by_user:
                    return StreamStatus.STOPPED_BY_USER

                # UTF-8 lossy — garbled bytes become U+FFFD, stream stays alive
                line = raw_line.decode("utf-8", errors="replace").rstrip("\r\n")
                entry = parse_logcat_line(line)
                if entry is None:
                    continue

                if not matches(entry, self.filters):
                    continue

                if self._reconnect_attempts > 0:
                    self._reconnect_attempts = 0
                    self._emit(
                        StreamStatus.RECONNECTED,
                        "log stream reconnected — resuming",
                    )

                if self._paused:
                    continue

                self._lines_count += 1
                self._recent_messages.append(entry.message)
                formatted = self.formatter.format_entry(entry)
                # Blank separator line before each entry (Rust v2.1 parity)
                self.console.print()
                self.console.print(formatted, highlight=False)

                if self._save_handle:
                    self._save_handle.write(entry.raw + "\n")
                    self._save_handle.flush()

            return StreamStatus.LOGCAT_EXITED

        except asyncio.CancelledError:
            return StreamStatus.STOPPED_BY_USER
        except Exception as exc:
            self._emit(StreamStatus.IO_ERROR, f"stream I/O error: {exc}")
            return StreamStatus.IO_ERROR
        finally:
            await self._kill_process()

    async def _kill_process(self) -> None:
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

    def _emit(self, status: StreamStatus, msg: str) -> None:
        if self._status_cb:
            try:
                self._status_cb(status, msg)
            except Exception:
                pass

    async def restart_with_new_pid(self, package: str) -> bool:
        pid = self.adb.get_pid(package)
        if pid:
            self.filters.update_pid(pid)
            return True
        return False
