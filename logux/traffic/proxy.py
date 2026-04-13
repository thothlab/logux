"""Traffic proxy adapter — integrates mitmproxy for HTTP/HTTPS inspection."""

from __future__ import annotations

import asyncio
import threading
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any

from rich.console import Console


@dataclass(slots=True)
class TrafficEntry:
    id: int
    timestamp: str
    method: str
    url: str
    host: str
    path: str
    status: int | None = None
    request_headers: dict[str, str] = field(default_factory=dict)
    request_body: bytes = b""
    response_headers: dict[str, str] = field(default_factory=dict)
    response_body: bytes = b""
    duration_ms: float = 0


@dataclass
class TrafficFilter:
    host: str = ""
    path: str = ""
    method: str = ""
    status: int | None = None
    body_search: str = ""

    def matches(self, entry: TrafficEntry) -> bool:
        if self.host and self.host.lower() not in entry.host.lower():
            return False
        if self.path and self.path.lower() not in entry.path.lower():
            return False
        if self.method and self.method.upper() != entry.method.upper():
            return False
        if self.status is not None and entry.status != self.status:
            return False
        if self.body_search:
            needle = self.body_search.lower().encode()
            if needle not in entry.request_body.lower() and needle not in entry.response_body.lower():
                return False
        return True

    def reset(self) -> None:
        self.host = ""
        self.path = ""
        self.method = ""
        self.status = None
        self.body_search = ""


class TrafficProxy:
    """Manages a mitmproxy instance for traffic interception."""

    def __init__(self, console: Console, listen_port: int = 8888) -> None:
        self.console = console
        self.listen_port = listen_port
        self.entries: list[TrafficEntry] = []
        self.filter = TrafficFilter()
        self._counter = 0
        self._running = False
        self._thread: threading.Thread | None = None
        self._master: Any = None
        self._mock_handler: Any = None

    @property
    def is_running(self) -> bool:
        return self._running

    def set_mock_handler(self, handler: Any) -> None:
        self._mock_handler = handler

    def start(self) -> tuple[bool, str]:
        if self._running:
            return False, "Proxy already running"

        try:
            from mitmproxy import options, master, http
            from mitmproxy.addons import default_addons
            from mitmproxy.proxy import mode_specs
        except ImportError:
            return False, "mitmproxy not installed. Run: pip install mitmproxy"

        self._running = True
        self._thread = threading.Thread(target=self._run_proxy, daemon=True)
        self._thread.start()
        return True, f"Proxy started on port {self.listen_port}"

    def _run_proxy(self) -> None:
        try:
            from mitmproxy import options as mopt
            from mitmproxy.tools.dump import DumpMaster

            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

            opts = mopt.Options(
                listen_port=self.listen_port,
                ssl_insecure=True,
            )
            self._master = DumpMaster(opts)
            self._master.addons.add(self._create_addon())
            loop.run_until_complete(self._master.run())
        except Exception as exc:
            self.console.print(f"[red]Proxy error: {exc}[/red]")
        finally:
            self._running = False

    def _create_addon(self) -> Any:
        proxy = self

        class TrafficAddon:
            def request(self, flow: Any) -> None:
                proxy._counter += 1
                entry = TrafficEntry(
                    id=proxy._counter,
                    timestamp=datetime.now().strftime("%H:%M:%S.%f")[:-3],
                    method=flow.request.method,
                    url=flow.request.pretty_url,
                    host=flow.request.pretty_host,
                    path=flow.request.path,
                    request_headers=dict(flow.request.headers),
                    request_body=flow.request.content or b"",
                )
                proxy.entries.append(entry)

                # Apply mock rules if handler is set
                if proxy._mock_handler:
                    mock_response = proxy._mock_handler.match_request(flow)
                    if mock_response:
                        flow.response = mock_response

            def response(self, flow: Any) -> None:
                for entry in reversed(proxy.entries):
                    if entry.url == flow.request.pretty_url and entry.status is None:
                        entry.status = flow.response.status_code
                        entry.response_headers = dict(flow.response.headers)
                        entry.response_body = flow.response.content or b""
                        break

        return TrafficAddon()

    def stop(self) -> tuple[bool, str]:
        if not self._running:
            return False, "Proxy not running"

        if self._master:
            self._master.shutdown()
        self._running = False
        return True, "Proxy stopped"

    def get_entries(self, limit: int = 50) -> list[TrafficEntry]:
        filtered = [e for e in self.entries if self.filter.matches(e)]
        return filtered[-limit:]

    def get_entry(self, entry_id: int) -> TrafficEntry | None:
        for e in self.entries:
            if e.id == entry_id:
                return e
        return None

    def clear(self) -> None:
        self.entries.clear()
        self._counter = 0
