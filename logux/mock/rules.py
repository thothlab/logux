"""Mock/Rewrite rules engine — YAML-based request matching and response override."""

from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml
from rich.console import Console


@dataclass
class MatchRule:
    method: str = ""
    path: str = ""
    host: str = ""
    query: dict[str, str] = field(default_factory=dict)
    headers: dict[str, str] = field(default_factory=dict)
    body_contains: str = ""


@dataclass
class MockResponse:
    type: str = "json"  # json, file, error, empty
    status: int = 200
    body: str = ""
    file: str = ""
    headers: dict[str, str] = field(default_factory=dict)
    delay_ms: int = 0


@dataclass
class MockRule:
    id: str
    enabled: bool = True
    match: MatchRule = field(default_factory=MatchRule)
    response: MockResponse = field(default_factory=MockResponse)
    hit_count: int = 0
    priority: int = 0


class MockEngine:
    """Loads YAML rules, matches requests, generates mock responses."""

    def __init__(self, console: Console) -> None:
        self.console = console
        self.rules: list[MockRule] = []
        self._yaml_path: Path | None = None
        self._last_modified: float = 0

    def load(self, yaml_path: str) -> tuple[bool, str]:
        path = Path(yaml_path)
        if not path.exists():
            return False, f"File not found: {yaml_path}"

        try:
            data = yaml.safe_load(path.read_text(encoding="utf-8"))
        except yaml.YAMLError as exc:
            return False, f"YAML parse error: {exc}"

        self._yaml_path = path
        self._last_modified = path.stat().st_mtime
        return self._parse_rules(data)

    def _parse_rules(self, data: dict[str, Any]) -> tuple[bool, str]:
        if not isinstance(data, dict) or "rules" not in data:
            return False, "Invalid format: expected 'rules' key at root"

        self.rules.clear()
        for i, rule_data in enumerate(data["rules"]):
            try:
                rule = self._parse_rule(rule_data, i)
                self.rules.append(rule)
            except Exception as exc:
                return False, f"Error parsing rule #{i}: {exc}"

        # Sort by priority (higher first)
        self.rules.sort(key=lambda r: r.priority, reverse=True)
        return True, f"Loaded {len(self.rules)} rules"

    def _parse_rule(self, data: dict[str, Any], index: int) -> MockRule:
        match_data = data.get("match", {})
        resp_data = data.get("response", {})

        match_rule = MatchRule(
            method=str(match_data.get("method", "")).upper(),
            path=str(match_data.get("path", "")),
            host=str(match_data.get("host", "")),
            query={str(k): str(v) for k, v in match_data.get("query", {}).items()},
            headers={str(k): str(v) for k, v in match_data.get("headers", {}).items()},
            body_contains=str(match_data.get("body_contains", "")),
        )

        response = MockResponse(
            type=str(resp_data.get("type", "json")),
            status=int(resp_data.get("status", 200)),
            body=str(resp_data.get("body", "")),
            file=str(resp_data.get("file", "")),
            headers={str(k): str(v) for k, v in resp_data.get("headers", {}).items()},
            delay_ms=int(resp_data.get("delay_ms", 0)),
        )

        return MockRule(
            id=str(data.get("id", f"rule_{index}")),
            enabled=bool(data.get("enabled", True)),
            match=match_rule,
            response=response,
            priority=int(data.get("priority", 0)),
        )

    def reload(self) -> tuple[bool, str]:
        if not self._yaml_path:
            return False, "No rules file loaded"
        return self.load(str(self._yaml_path))

    def check_hot_reload(self) -> bool:
        """Check if the YAML file has been modified and reload if so."""
        if not self._yaml_path or not self._yaml_path.exists():
            return False
        mtime = self._yaml_path.stat().st_mtime
        if mtime > self._last_modified:
            ok, msg = self.reload()
            if ok:
                self.console.print(f"[yellow]Rules hot-reloaded: {msg}[/yellow]")
            return ok
        return False

    def enable_rule(self, rule_id: str) -> bool:
        for rule in self.rules:
            if rule.id == rule_id:
                rule.enabled = True
                return True
        return False

    def disable_rule(self, rule_id: str) -> bool:
        for rule in self.rules:
            if rule.id == rule_id:
                rule.enabled = False
                return True
        return False

    def get_rule(self, rule_id: str) -> MockRule | None:
        for rule in self.rules:
            if rule.id == rule_id:
                return rule
        return None

    def match_request(self, flow: Any) -> Any:
        """Match a mitmproxy flow against rules and return a Response or None."""
        for rule in self.rules:
            if not rule.enabled:
                continue
            if self._matches_flow(rule.match, flow):
                rule.hit_count += 1
                self.console.print(
                    f"[magenta]Mock applied: {rule.id} → "
                    f"{flow.request.method} {flow.request.path}[/magenta]"
                )
                return self._build_response(rule.response, flow)
        return None

    def _matches_flow(self, match: MatchRule, flow: Any) -> bool:
        req = flow.request

        if match.method and req.method.upper() != match.method:
            return False
        if match.path and match.path not in req.path:
            return False
        if match.host and match.host.lower() not in req.pretty_host.lower():
            return False

        # Query params
        if match.query:
            query = dict(req.query or {})
            for k, v in match.query.items():
                if query.get(k) != v:
                    return False

        # Headers
        if match.headers:
            for k, v in match.headers.items():
                if req.headers.get(k, "") != v:
                    return False

        # Body contains
        if match.body_contains:
            body = (req.content or b"").decode("utf-8", errors="replace")
            if match.body_contains not in body:
                return False

        return True

    def _build_response(self, resp: MockResponse, flow: Any) -> Any:
        from mitmproxy import http

        if resp.delay_ms > 0:
            time.sleep(resp.delay_ms / 1000.0)

        headers = {"Content-Type": "application/json", **resp.headers}

        if resp.type == "file":
            file_path = Path(resp.file)
            if not file_path.is_absolute() and self._yaml_path:
                file_path = self._yaml_path.parent / file_path
            if file_path.exists():
                content = file_path.read_bytes()
            else:
                content = json.dumps({"error": f"Mock file not found: {resp.file}"}).encode()
                headers["Content-Type"] = "application/json"
        elif resp.type == "error":
            content = json.dumps({"error": "Mocked error", "status": resp.status}).encode()
        elif resp.type == "empty":
            content = b""
        else:
            content = resp.body.encode("utf-8") if resp.body else b"{}"

        return http.Response.make(
            status_code=resp.status,
            content=content,
            headers=headers,
        )
