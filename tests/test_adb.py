"""Tests for ADB client (offline tests, no real device needed)."""

from logux.adb.client import Device, DeviceState, ConnectionType


def test_device_usb():
    dev = Device(serial="ABCD1234", state=DeviceState.DEVICE, model="Pixel")
    assert dev.connection_type == ConnectionType.USB
    assert dev.is_online is True
    assert "Pixel" in dev.display_name
    assert "USB" in dev.display_name


def test_device_tcp():
    dev = Device(serial="192.168.1.10:5555", state=DeviceState.DEVICE, model="Galaxy")
    assert dev.connection_type == ConnectionType.TCP
    assert "TCP" in dev.display_name


def test_device_offline():
    dev = Device(serial="ABCD", state=DeviceState.OFFLINE)
    assert dev.is_online is False


def test_device_display_name_fallback():
    dev = Device(serial="XYZ", state=DeviceState.DEVICE)
    assert "XYZ" in dev.display_name
