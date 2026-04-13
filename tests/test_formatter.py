"""Tests for log formatter."""

from logux.logs.parser import LogEntry, LogLevel
from logux.logs.formatter import LogFormatter, FormatConfig, Preset


def _make_entry(**kwargs):
    defaults = dict(
        timestamp="04-13 12:00:00.000",
        pid=100,
        tid=200,
        level=LogLevel.INFO,
        tag="TestTag",
        message="Hello World",
        raw="raw",
    )
    defaults.update(kwargs)
    return LogEntry(**defaults)


def test_json_format():
    fmt = LogFormatter()
    fmt.config.apply_preset(Preset.JSON)
    entry = _make_entry()
    text = fmt.format_entry(entry)
    plain = text.plain
    assert '"tag": "TestTag"' in plain
    assert '"message": "Hello World"' in plain


def test_compact_format():
    fmt = LogFormatter()
    fmt.config.apply_preset(Preset.COMPACT)
    entry = _make_entry()
    text = fmt.format_entry(entry)
    plain = text.plain
    assert "Hello World" in plain
    assert "TestTag" in plain


def test_minimal_format():
    fmt = LogFormatter()
    fmt.config.apply_preset(Preset.MINIMAL)
    entry = _make_entry()
    text = fmt.format_entry(entry)
    plain = text.plain
    assert "12:00:00" not in plain  # no timestamp in minimal
    assert "Hello World" in plain


def test_highlight_text():
    fmt = LogFormatter()
    fmt.highlight_text = "World"
    entry = _make_entry(message="Hello World!")
    text = fmt.format_entry(entry)
    plain = text.plain
    assert "Hello World!" in plain


def test_toggle_field():
    cfg = FormatConfig()
    assert cfg.toggle_field("pid", True) is True
    assert cfg.pid is True
    assert cfg.toggle_field("nonexistent", True) is False
