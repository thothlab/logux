"""Tests for mock rules engine."""

import tempfile
from pathlib import Path

from rich.console import Console

from logux.mock.rules import MockEngine


SAMPLE_YAML = """
rules:
  - id: test_mock
    enabled: true
    priority: 10
    match:
      method: GET
      path: /api/test
      query:
        key: "value"
    response:
      type: json
      status: 200
      body: '{"ok": true}'

  - id: disabled_mock
    enabled: false
    match:
      path: /api/other
    response:
      type: error
      status: 500
"""


def test_load_rules():
    console = Console(quiet=True)
    engine = MockEngine(console)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(SAMPLE_YAML)
        f.flush()
        ok, msg = engine.load(f.name)

    assert ok is True
    assert len(engine.rules) == 2


def test_enable_disable():
    console = Console(quiet=True)
    engine = MockEngine(console)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(SAMPLE_YAML)
        f.flush()
        engine.load(f.name)

    assert engine.disable_rule("test_mock") is True
    rule = engine.get_rule("test_mock")
    assert rule is not None
    assert rule.enabled is False

    assert engine.enable_rule("test_mock") is True
    assert rule.enabled is True

    assert engine.enable_rule("nonexistent") is False


def test_priority_order():
    console = Console(quiet=True)
    engine = MockEngine(console)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(SAMPLE_YAML)
        f.flush()
        engine.load(f.name)

    # Higher priority first
    assert engine.rules[0].id == "test_mock"
    assert engine.rules[0].priority == 10


def test_reload():
    console = Console(quiet=True)
    engine = MockEngine(console)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(SAMPLE_YAML)
        f.flush()
        engine.load(f.name)

        ok, msg = engine.reload()
        assert ok is True


def test_invalid_yaml():
    console = Console(quiet=True)
    engine = MockEngine(console)

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write("not: a: valid: yaml: [")
        f.flush()
        ok, msg = engine.load(f.name)

    assert ok is False


def test_missing_file():
    console = Console(quiet=True)
    engine = MockEngine(console)
    ok, msg = engine.load("/nonexistent/path.yaml")
    assert ok is False
