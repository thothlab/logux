"""ADB client — device management and logcat streaming."""

from __future__ import annotations

import asyncio
import shutil
import subprocess
from dataclasses import dataclass, field
from enum import Enum


class DeviceState(Enum):
    DEVICE = "device"
    OFFLINE = "offline"
    UNAUTHORIZED = "unauthorized"
    NO_DEVICE = "no device"


class ConnectionType(Enum):
    USB = "usb"
    TCP = "tcp"


@dataclass
class Device:
    serial: str
    state: DeviceState
    model: str = ""
    product: str = ""
    transport_id: str = ""

    @property
    def connection_type(self) -> ConnectionType:
        if ":" in self.serial:
            return ConnectionType.TCP
        return ConnectionType.USB

    @property
    def display_name(self) -> str:
        name = self.model or self.product or self.serial
        conn = "TCP" if self.connection_type == ConnectionType.TCP else "USB"
        return f"{name} [{conn}] ({self.serial})"

    @property
    def is_online(self) -> bool:
        return self.state == DeviceState.DEVICE


@dataclass
class ADBClient:
    """Manages ADB connections and provides logcat streaming."""

    adb_path: str = ""
    selected_device: Device | None = None
    _devices: list[Device] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not self.adb_path:
            self.adb_path = shutil.which("adb") or "adb"

    def _run(self, *args: str, device: str | None = None) -> subprocess.CompletedProcess[str]:
        cmd = [self.adb_path]
        if device:
            cmd.extend(["-s", device])
        cmd.extend(args)
        return subprocess.run(cmd, capture_output=True, text=True, timeout=10)

    def _device_serial(self) -> str | None:
        if self.selected_device:
            return self.selected_device.serial
        return None

    # --- Device management ---

    def list_devices(self) -> list[Device]:
        result = self._run("devices", "-l")
        devices: list[Device] = []
        for line in result.stdout.strip().splitlines()[1:]:
            line = line.strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) < 2:
                continue
            serial = parts[0]
            try:
                state = DeviceState(parts[1])
            except ValueError:
                state = DeviceState.OFFLINE

            model = product = transport_id = ""
            for part in parts[2:]:
                if part.startswith("model:"):
                    model = part.split(":", 1)[1]
                elif part.startswith("product:"):
                    product = part.split(":", 1)[1]
                elif part.startswith("transport_id:"):
                    transport_id = part.split(":", 1)[1]

            devices.append(Device(
                serial=serial,
                state=state,
                model=model,
                product=product,
                transport_id=transport_id,
            ))
        self._devices = devices
        return devices

    def select_device(self, serial: str) -> Device | None:
        devices = self.list_devices()
        for dev in devices:
            if dev.serial == serial:
                self.selected_device = dev
                return dev
        return None

    def auto_select(self) -> Device | None:
        devices = self.list_devices()
        online = [d for d in devices if d.is_online]
        if len(online) == 1:
            self.selected_device = online[0]
            return online[0]
        return None

    def connect_tcp(self, address: str) -> tuple[bool, str]:
        if ":" not in address:
            address = f"{address}:5555"
        result = self._run("connect", address)
        output = result.stdout.strip()
        success = "connected" in output.lower()
        if success:
            self.list_devices()
            self.select_device(address)
        return success, output

    def disconnect(self, serial: str | None = None) -> tuple[bool, str]:
        args = ["disconnect"]
        if serial:
            args.append(serial)
        elif self.selected_device and self.selected_device.connection_type == ConnectionType.TCP:
            args.append(self.selected_device.serial)
        result = self._run(*args)
        output = result.stdout.strip()
        if self.selected_device and (not serial or serial == self.selected_device.serial):
            self.selected_device = None
        return True, output

    # --- Package utilities ---

    def get_pid(self, package: str) -> int | None:
        serial = self._device_serial()
        result = self._run("shell", f"pidof {package}", device=serial)
        pid_str = result.stdout.strip()
        if pid_str and pid_str.isdigit():
            return int(pid_str)
        # Fallback: parse ps output
        result = self._run("shell", f"ps -A | grep {package}", device=serial)
        for line in result.stdout.strip().splitlines():
            parts = line.split()
            if len(parts) >= 2 and package in line:
                try:
                    return int(parts[1])
                except ValueError:
                    continue
        return None

    # --- Logcat streaming ---

    async def stream_logcat(
        self,
        buffer: str = "main",
        clear_first: bool = False,
    ) -> asyncio.subprocess.Process:
        serial = self._device_serial()
        if clear_first:
            cmd = [self.adb_path]
            if serial:
                cmd.extend(["-s", serial])
            cmd.extend(["logcat", "-c"])
            await asyncio.create_subprocess_exec(*cmd)

        cmd = [self.adb_path]
        if serial:
            cmd.extend(["-s", serial])
        cmd.extend(["logcat", "-v", "threadtime", "-b", buffer])

        process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        return process

    def check_adb(self) -> tuple[bool, str]:
        try:
            result = self._run("version")
            return True, result.stdout.strip()
        except FileNotFoundError:
            return False, "adb not found in PATH"
        except subprocess.TimeoutExpired:
            return False, "adb command timed out"

    # --- Server control (for /reconnect) ---

    def kill_server(self) -> tuple[bool, str]:
        try:
            result = self._run("kill-server")
            msg = (result.stdout + result.stderr).strip() or "adb server killed"
            return result.returncode == 0, msg
        except (FileNotFoundError, subprocess.TimeoutExpired) as e:
            return False, f"adb kill-server failed: {e}"

    def start_server(self) -> tuple[bool, str]:
        try:
            result = self._run("start-server")
            msg = (result.stdout + result.stderr).strip() or "adb server started"
            return result.returncode == 0, msg
        except (FileNotFoundError, subprocess.TimeoutExpired) as e:
            return False, f"adb start-server failed: {e}"

    def get_foreground_package(self) -> str | None:
        """Return the currently focused app's package name, or None."""
        serial = self._device_serial()
        try:
            result = self._run("shell", "dumpsys", "activity", "activities", device=serial)
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return None
        for line in result.stdout.splitlines():
            if "mResumedActivity" in line or "topResumedActivity" in line:
                for tok in line.split():
                    if "/" in tok:
                        pkg = tok.split("/", 1)[0]
                        if "." in pkg:
                            return pkg
        return None
