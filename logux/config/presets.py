"""Preset management — save/load filter+format configurations."""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

from ..logs.filters import FilterState
from ..logs.formatter import FormatConfig, Preset
from ..logs.parser import LogLevel


DEFAULT_PRESETS_DIR = Path.home() / ".logux" / "presets"


def _ensure_dir() -> Path:
    DEFAULT_PRESETS_DIR.mkdir(parents=True, exist_ok=True)
    return DEFAULT_PRESETS_DIR


def save_preset(
    name: str,
    filters: FilterState,
    format_config: FormatConfig,
) -> Path:
    """Save current filter + format state as a named preset."""
    _ensure_dir()
    data = {
        "name": name,
        "filters": {
            "package": filters.package,
            "tags": list(filters.tags),
            "min_level": filters.min_level.value,
            "text": filters.text,
            "regex": filters.regex.pattern if filters.regex else None,
        },
        "format": {
            "preset": format_config.preset.value,
            "timestamp": format_config.timestamp,
            "level": format_config.level,
            "tag": format_config.tag,
            "pid": format_config.pid,
            "tid": format_config.tid,
            "message": format_config.message,
        },
    }
    path = DEFAULT_PRESETS_DIR / f"{name}.json"
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    return path


def load_preset(
    name: str,
    filters: FilterState,
    format_config: FormatConfig,
) -> bool:
    """Load a named preset into the given filter/format state. Returns True on success."""
    path = DEFAULT_PRESETS_DIR / f"{name}.json"
    if not path.exists():
        return False

    data = json.loads(path.read_text(encoding="utf-8"))

    # Apply filters
    f = data.get("filters", {})
    if f.get("package"):
        filters.package = f["package"]
    filters.tags = set(f.get("tags", []))
    filters.min_level = LogLevel(f.get("min_level", 0))
    filters.text = f.get("text", "")
    if f.get("regex"):
        filters.set_regex(f["regex"])
    else:
        filters.regex = None

    # Apply format
    fmt = data.get("format", {})
    if fmt.get("preset"):
        format_config.apply_preset(Preset(fmt["preset"]))
    for field_name in ("timestamp", "level", "tag", "pid", "tid", "message"):
        if field_name in fmt:
            setattr(format_config, field_name, fmt[field_name])

    return True


def list_presets() -> list[str]:
    """List all saved preset names."""
    _ensure_dir()
    return sorted(p.stem for p in DEFAULT_PRESETS_DIR.glob("*.json"))


def delete_preset(name: str) -> bool:
    path = DEFAULT_PRESETS_DIR / f"{name}.json"
    if path.exists():
        path.unlink()
        return True
    return False
