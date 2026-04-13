"""Tests for filter engine."""

from logux.logs.parser import LogEntry, LogLevel
from logux.logs.filters import FilterState, matches


def _make_entry(**kwargs):
    defaults = dict(
        timestamp="04-13 12:00:00.000",
        pid=100,
        tid=200,
        level=LogLevel.DEBUG,
        tag="TestTag",
        message="test message",
        raw="raw line",
    )
    defaults.update(kwargs)
    return LogEntry(**defaults)


def test_no_filters_passes_all():
    state = FilterState()
    entry = _make_entry()
    assert matches(entry, state) is True


def test_level_filter():
    state = FilterState()
    state.set_level(LogLevel.WARN)
    assert matches(_make_entry(level=LogLevel.DEBUG), state) is False
    assert matches(_make_entry(level=LogLevel.WARN), state) is True
    assert matches(_make_entry(level=LogLevel.ERROR), state) is True


def test_pid_filter():
    state = FilterState()
    state.set_pid(100)
    assert matches(_make_entry(pid=100), state) is True
    assert matches(_make_entry(pid=200), state) is False


def test_tag_filter():
    state = FilterState()
    state.add_tag("Test")
    assert matches(_make_entry(tag="TestTag"), state) is True
    assert matches(_make_entry(tag="OtherTag"), state) is False


def test_text_filter():
    state = FilterState()
    state.set_text("hello")
    assert matches(_make_entry(message="Hello World"), state) is True
    assert matches(_make_entry(message="Goodbye"), state) is False


def test_regex_filter():
    state = FilterState()
    state.set_regex(r"error\d+")
    assert matches(_make_entry(message="found error123"), state) is True
    assert matches(_make_entry(message="no match here"), state) is False


def test_thread_filter():
    state = FilterState()
    state.set_threads({200})
    assert matches(_make_entry(tid=200), state) is True
    assert matches(_make_entry(tid=300), state) is False


def test_combined_filters():
    state = FilterState()
    state.set_level(LogLevel.INFO)
    state.add_tag("Net")
    state.set_text("request")

    entry = _make_entry(level=LogLevel.INFO, tag="Network", message="HTTP request sent")
    assert matches(entry, state) is True

    entry = _make_entry(level=LogLevel.DEBUG, tag="Network", message="HTTP request sent")
    assert matches(entry, state) is False  # level too low


def test_reset():
    state = FilterState()
    state.set_pid(100)
    state.add_tag("X")
    state.set_level(LogLevel.ERROR)
    state.reset()
    assert matches(_make_entry(level=LogLevel.VERBOSE), state) is True


def test_package_tracking():
    state = FilterState()
    state.set_package("com.example.app", pid=100)
    assert state._package_tracking is True
    assert matches(_make_entry(pid=100), state) is True
    assert matches(_make_entry(pid=200), state) is False

    state.update_pid(300)
    assert matches(_make_entry(pid=300), state) is True
    assert matches(_make_entry(pid=100), state) is False
