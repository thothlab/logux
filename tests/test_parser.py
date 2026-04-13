"""Tests for logcat parser."""

from logux.logs.parser import parse_logcat_line, LogLevel


def test_threadtime_format():
    line = "04-13 12:34:56.789  1234  5678 D MyTag   : Hello World"
    entry = parse_logcat_line(line)
    assert entry is not None
    assert entry.timestamp == "04-13 12:34:56.789"
    assert entry.pid == 1234
    assert entry.tid == 5678
    assert entry.level == LogLevel.DEBUG
    assert entry.tag == "MyTag"
    assert entry.message == "Hello World"


def test_threadtime_error():
    line = "04-13 12:34:56.789  1234  5678 E CrashTag: java.lang.NullPointerException"
    entry = parse_logcat_line(line)
    assert entry is not None
    assert entry.level == LogLevel.ERROR
    assert entry.tag == "CrashTag"
    assert "NullPointerException" in entry.message


def test_brief_format():
    line = "D/MyTag( 1234): Some debug message"
    entry = parse_logcat_line(line)
    assert entry is not None
    assert entry.level == LogLevel.DEBUG
    assert entry.tag == "MyTag"
    assert entry.pid == 1234
    assert entry.message == "Some debug message"


def test_header_line_skipped():
    line = "--------- beginning of main"
    entry = parse_logcat_line(line)
    assert entry is None


def test_empty_line():
    entry = parse_logcat_line("")
    assert entry is None


def test_unparseable_line():
    line = "some random text that is not logcat"
    entry = parse_logcat_line(line)
    assert entry is not None
    assert entry.message == line


def test_level_from_char():
    assert LogLevel.from_char("V") == LogLevel.VERBOSE
    assert LogLevel.from_char("D") == LogLevel.DEBUG
    assert LogLevel.from_char("I") == LogLevel.INFO
    assert LogLevel.from_char("W") == LogLevel.WARN
    assert LogLevel.from_char("E") == LogLevel.ERROR
    assert LogLevel.from_char("F") == LogLevel.FATAL


def test_level_char_property():
    assert LogLevel.VERBOSE.char == "V"
    assert LogLevel.ERROR.char == "E"
