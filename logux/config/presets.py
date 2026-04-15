"""Preset management + auto-saved filter history + per-app filter memory."""

from __future__ import annotations

import json
import time
from pathlib import Path

from ..logs.filters import FilterState
from ..logs.formatter import FormatConfig, Preset
from ..logs.parser import LogLevel


CONFIG_DIR = Path.home() / ".logux"
PRESETS_DIR = CONFIG_DIR / "presets"
FILTER_PRESETS_DIR = CONFIG_DIR / "filter_presets"
APP_FILTERS_DIR = CONFIG_DIR / "app_filters"
APP_HISTORY_FILE = CONFIG_DIR / "app_history.json"


def _ensure(*paths: Path) -> None:
    for p in paths:
        p.mkdir(parents=True, exist_ok=True)


# ---------------------------------------------------------------------------
# Named format+filter presets (/preset save|load|list|delete)
# ---------------------------------------------------------------------------

def save_preset(
    name: str,
    filters: FilterState,
    format_config: FormatConfig,
) -> Path:
    _ensure(PRESETS_DIR)
    data = {
        "name": name,
        "filters": _filters_to_dict(filters),
        "format": _format_to_dict(format_config),
    }
    path = PRESETS_DIR / f"{name}.json"
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    return path


def load_preset(
    name: str,
    filters: FilterState,
    format_config: FormatConfig,
) -> bool:
    path = PRESETS_DIR / f"{name}.json"
    if not path.exists():
        return False

    data = json.loads(path.read_text(encoding="utf-8"))
    _apply_filters_dict(filters, data.get("filters", {}))
    _apply_format_dict(format_config, data.get("format", {}))
    return True


def list_presets() -> list[str]:
    _ensure(PRESETS_DIR)
    return sorted(p.stem for p in PRESETS_DIR.glob("*.json"))


def delete_preset(name: str) -> bool:
    path = PRESETS_DIR / f"{name}.json"
    if path.exists():
        path.unlink()
        return True
    return False


# ---------------------------------------------------------------------------
# Auto-saved filter presets (every /filter set is kept for tab-completion)
# ---------------------------------------------------------------------------

def save_filter_preset(expr: str) -> None:
    """Save a filter expression under an auto-generated name. De-duped on expr."""
    if not expr.strip():
        return
    _ensure(FILTER_PRESETS_DIR)
    for existing in FILTER_PRESETS_DIR.glob("*.json"):
        try:
            data = json.loads(existing.read_text(encoding="utf-8"))
            if data.get("expr") == expr:
                return
        except Exception:
            continue
    name = f"auto-{int(time.time())}"
    (FILTER_PRESETS_DIR / f"{name}.json").write_text(
        json.dumps({"name": name, "expr": expr}, ensure_ascii=False),
        encoding="utf-8",
    )


def list_filter_presets() -> list[tuple[str, str]]:
    """Return [(name, expr), …] of saved filter expressions."""
    _ensure(FILTER_PRESETS_DIR)
    out: list[tuple[str, str]] = []
    for path in sorted(FILTER_PRESETS_DIR.glob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
            out.append((data.get("name", path.stem), data.get("expr", "")))
        except Exception:
            continue
    return out


# ---------------------------------------------------------------------------
# Per-app filter memory
# ---------------------------------------------------------------------------

def save_app_filters(package: str, filters: FilterState) -> None:
    if not package:
        return
    _ensure(APP_FILTERS_DIR)
    data = _filters_to_dict(filters)
    (APP_FILTERS_DIR / f"{_safe_name(package)}.json").write_text(
        json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8"
    )


def load_app_filters(package: str, filters: FilterState) -> bool:
    path = APP_FILTERS_DIR / f"{_safe_name(package)}.json"
    if not path.exists():
        return False
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return False
    _apply_filters_dict(filters, data)
    return True


def _safe_name(package: str) -> str:
    return package.replace("/", "_").replace(":", "_")


# ---------------------------------------------------------------------------
# App history — packages seen via /app, for tab-completion
# ---------------------------------------------------------------------------

def load_app_history() -> list[str]:
    if not APP_HISTORY_FILE.exists():
        return []
    try:
        return json.loads(APP_HISTORY_FILE.read_text(encoding="utf-8"))
    except Exception:
        return []


def save_app_history(items: list[str]) -> None:
    _ensure(CONFIG_DIR)
    APP_HISTORY_FILE.write_text(
        json.dumps(items[-100:], ensure_ascii=False), encoding="utf-8"
    )


def push_app_history(package: str) -> None:
    items = load_app_history()
    if package in items:
        items.remove(package)
    items.append(package)
    save_app_history(items)


# ---------------------------------------------------------------------------
# /forget — wipe auto-saved filters + per-app memory + history
# ---------------------------------------------------------------------------

def clear_saved_filters() -> tuple[int, int, int]:
    """Return (filter_presets_removed, app_states_removed, history_entries_removed)."""
    _ensure(FILTER_PRESETS_DIR, APP_FILTERS_DIR)
    p = sum(1 for f in FILTER_PRESETS_DIR.glob("*.json"))
    for f in FILTER_PRESETS_DIR.glob("*.json"):
        f.unlink()
    a = sum(1 for f in APP_FILTERS_DIR.glob("*.json"))
    for f in APP_FILTERS_DIR.glob("*.json"):
        f.unlink()
    h = 0
    if APP_HISTORY_FILE.exists():
        try:
            h = len(json.loads(APP_HISTORY_FILE.read_text(encoding="utf-8")))
        except Exception:
            pass
        APP_HISTORY_FILE.unlink()
    return p, a, h


# ---------------------------------------------------------------------------
# Serialization helpers
# ---------------------------------------------------------------------------

def _filters_to_dict(f: FilterState) -> dict:
    return {
        "package": f.package,
        "tags": sorted(f.tags),
        "min_level": f.min_level.value,
        "text": f.text,
        "msgs": sorted(f.msgs),
        "regex": f.regex.pattern if f.regex else None,
        "exclude_tags": sorted(f.exclude_tags),
        "exclude_msgs": sorted(f.exclude_msgs),
    }


def _apply_filters_dict(f: FilterState, d: dict) -> None:
    if d.get("package"):
        f.package = d["package"]
    f.tags = set(d.get("tags", []))
    f.min_level = LogLevel(d.get("min_level", 0))
    f.text = d.get("text", "")
    f.msgs = set(d.get("msgs", []))
    if d.get("regex"):
        try:
            f.set_regex(d["regex"])
        except Exception:
            f.regex = None
    else:
        f.regex = None
    f.exclude_tags = set(d.get("exclude_tags", []))
    f.exclude_msgs = set(d.get("exclude_msgs", []))


def _format_to_dict(fmt: FormatConfig) -> dict:
    return {
        "preset": fmt.preset.value,
        "timestamp": fmt.timestamp,
        "level": fmt.level,
        "tag": fmt.tag,
        "pid": fmt.pid,
        "tid": fmt.tid,
        "message": fmt.message,
    }


def _apply_format_dict(fmt: FormatConfig, d: dict) -> None:
    if d.get("preset"):
        try:
            fmt.apply_preset(Preset(d["preset"]))
        except ValueError:
            pass
    for name in ("timestamp", "level", "tag", "pid", "tid", "message"):
        if name in d:
            setattr(fmt, name, d[name])
