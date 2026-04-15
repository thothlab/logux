"""Command dispatcher — handles all /commands from the shell."""

from __future__ import annotations

from typing import TYPE_CHECKING

from rich.table import Table
from rich.panel import Panel
from rich.text import Text

from ..logs.parser import LogLevel
from ..logs.formatter import Preset
from ..config.presets import (
    save_preset,
    load_preset,
    list_presets,
    delete_preset,
    save_filter_preset,
    list_filter_presets,
    save_app_filters,
    load_app_filters,
    push_app_history,
    clear_saved_filters,
)

if TYPE_CHECKING:
    from .shell import LoguxShell


async def dispatch(shell: LoguxShell, raw_input: str) -> None:
    """Parse and execute a /command."""
    parts = raw_input.strip().split(maxsplit=1)
    if not parts:
        return

    cmd = parts[0].lower()
    args = parts[1].strip() if len(parts) > 1 else ""

    handlers = {
        "/help": cmd_help,
        "/exit": cmd_exit,
        "/clear": cmd_clear,
        "/devices": cmd_devices,
        "/connect": cmd_connect,
        "/disconnect": cmd_disconnect,
        "/reconnect": cmd_reconnect,
        "/app": cmd_app,
        "/pid": cmd_pid,
        "/tag": cmd_tag,
        "/level": cmd_level,
        "/grep": cmd_grep,
        "/msg": cmd_msg,
        "/regex": cmd_regex,
        "/exclude": cmd_exclude,
        "/filter": cmd_filter,
        "/format": cmd_format,
        "/fields": cmd_fields,
        "/pause": cmd_pause,
        "/resume": cmd_resume,
        "/stop": cmd_stop,
        "/save": cmd_save,
        "/copy": cmd_copy,
        "/preset": cmd_preset,
        "/forget": cmd_forget,
        "/traffic": cmd_traffic,
        "/mock": cmd_mock,
    }

    handler = handlers.get(cmd)
    if handler:
        await handler(shell, args)
    else:
        shell.console.print(f"[red]Unknown command: {cmd}[/red] — type /help")


# --- General ---

async def cmd_help(shell: LoguxShell, args: str) -> None:
    table = Table(title="logux commands", show_header=True, header_style="bold cyan")
    table.add_column("Command", style="green", min_width=28)
    table.add_column("Description")

    table.add_row("/help", "Show this help")
    table.add_row("/exit", "Exit logux")
    table.add_row("/clear", "Clear screen")
    table.add_row("", "")
    table.add_row("[bold]ADB[/bold]", "")
    table.add_row("/devices", "List connected devices (pick by number)")
    table.add_row("/connect <ip:port>", "Connect to TCP device")
    table.add_row("/disconnect", "Disconnect current device")
    table.add_row("/reconnect", "Hard-reset adb server + restart log stream")
    table.add_row("", "")
    table.add_row("[bold]Logs[/bold]", "")
    table.add_row("/app <package>", "Filter by app (PID tracking + auto-restore filters)")
    table.add_row("/pid <pid>", "Filter by PID")
    table.add_row("/tag <tag> | -<tag> | reset", "Add / remove / clear tag filter")
    table.add_row("/level <V|D|I|W|E|F> | reset", "Minimum log level")
    table.add_row("/grep <text> | reset", "Filter by text (tag + msg, case-insensitive)")
    table.add_row("/msg <text> | reset", "Message-only OR filter (repeatable)")
    table.add_row("/regex <pattern> | reset", "Filter by regex")
    table.add_row("/exclude tag|msg <v>", "Hide matching tags or messages")
    table.add_row("/exclude show|reset|remove <v>", "Manage exclusions")
    table.add_row("", "")
    table.add_row("[bold]Filter ops[/bold]", "")
    table.add_row("/filter show", "Show active filters")
    table.add_row("/filter reset", "Clear all filters")
    table.add_row("/filter edit", "Show current filter expression (for copy-editing)")
    table.add_row("/filter set <expr>", "Replace filters from expression")
    table.add_row("", "")
    table.add_row("[bold]Format[/bold]", "")
    table.add_row("/format <preset>", "compact | threadtime | verbose | minimal | json")
    table.add_row("/fields +field -field", "Toggle fields: timestamp, level, tag, pid, tid")
    table.add_row("", "")
    table.add_row("[bold]Control[/bold]", "")
    table.add_row("/pause", "Toggle pause (logs still captured)")
    table.add_row("/resume", "Resume log output")
    table.add_row("/stop", "Stop log stream (no auto-reconnect)")
    table.add_row("/save <file> | off", "Save matching logs to file; 'off' to stop")
    table.add_row("/copy [N]", "Copy last N messages to clipboard (default 100)")
    table.add_row("", "")
    table.add_row("[bold]Presets / memory[/bold]", "")
    table.add_row("/preset save|load|list|delete <name>", "Named filter+format presets")
    table.add_row("/forget", "Wipe auto-saved filters + per-app memory + history")
    table.add_row("", "")
    table.add_row("[bold]Traffic[/bold]", "")
    table.add_row("/traffic open", "Start proxy for traffic inspection")
    table.add_row("/traffic close", "Stop proxy")
    table.add_row("/traffic list", "Show captured requests")
    table.add_row("/traffic inspect <id>", "Inspect a request/response")
    table.add_row("/traffic filter <expr>", "Filter: host=, path=, method=, status=")
    table.add_row("/traffic clear", "Clear captured traffic")
    table.add_row("", "")
    table.add_row("[bold]Mock[/bold]", "")
    table.add_row("/mock load <file.yaml>", "Load mock rules")
    table.add_row("/mock list", "List loaded rules")
    table.add_row("/mock enable <id>", "Enable a rule")
    table.add_row("/mock disable <id>", "Disable a rule")
    table.add_row("/mock reload", "Reload rules from file")

    shell.console.print(table)


async def cmd_exit(shell: LoguxShell, args: str) -> None:
    shell.request_exit()


async def cmd_clear(shell: LoguxShell, args: str) -> None:
    shell.console.clear()


# --- ADB ---

async def cmd_devices(shell: LoguxShell, args: str) -> None:
    devices = shell.adb.list_devices()
    if not devices:
        shell.console.print("[yellow]No devices found[/yellow]")
        return

    # Numeric-select mode: /devices 2
    if args.strip().isdigit():
        idx = int(args.strip()) - 1
        if 0 <= idx < len(devices):
            dev = devices[idx]
            shell.adb.selected_device = dev
            shell.console.print(f"[green]Selected: {dev.display_name}[/green]")
            await _auto_start_stream(shell)
        else:
            shell.console.print(f"[red]Invalid index: {args}[/red]")
        return

    table = Table(show_header=True, header_style="bold")
    table.add_column("#", style="dim", width=3)
    table.add_column("Serial", style="cyan")
    table.add_column("State")
    table.add_column("Model")
    table.add_column("Type")
    table.add_column("Selected")

    for i, dev in enumerate(devices, start=1):
        state_style = "green" if dev.is_online else "red"
        selected = "→" if shell.adb.selected_device and dev.serial == shell.adb.selected_device.serial else ""
        table.add_row(
            str(i),
            dev.serial,
            Text(dev.state.value, style=state_style),
            dev.model or dev.product or "—",
            dev.connection_type.value.upper(),
            selected,
        )

    shell.console.print(table)
    shell.console.print("[dim]Tip: /devices <N> to select by number[/dim]")


async def cmd_connect(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /connect <ip:port>[/red]")
        return
    ok, msg = shell.adb.connect_tcp(args)
    style = "green" if ok else "red"
    shell.console.print(f"[{style}]{msg}[/{style}]")
    if ok:
        await _auto_start_stream(shell)


async def cmd_disconnect(shell: LoguxShell, args: str) -> None:
    ok, msg = shell.adb.disconnect(args or None)
    shell.console.print(f"[yellow]{msg}[/yellow]")
    if shell.log_stream.is_running:
        await shell.log_stream.stop()
        shell.console.print("[yellow]Log stream stopped[/yellow]")


async def cmd_reconnect(shell: LoguxShell, args: str) -> None:
    """Hard-reset adb server, refresh devices, and restart the log stream."""
    if shell.log_stream.is_running:
        await shell.log_stream.stop()
        shell.console.print("[dim]Log stream stopped for reconnect[/dim]")

    ok, msg = shell.adb.kill_server()
    shell.console.print(f"[{'yellow' if ok else 'red'}]kill-server: {msg}[/{'yellow' if ok else 'red'}]")

    ok, msg = shell.adb.start_server()
    shell.console.print(f"[{'green' if ok else 'red'}]start-server: {msg}[/{'green' if ok else 'red'}]")

    devices = shell.adb.list_devices()
    online = [d for d in devices if d.is_online]
    shell.console.print(f"[dim]Devices: {len(online)} online / {len(devices)} total[/dim]")

    if shell.adb.selected_device:
        still_there = next(
            (d for d in devices if d.serial == shell.adb.selected_device.serial),
            None,
        )
        if still_there:
            shell.adb.selected_device = still_there
        else:
            shell.adb.selected_device = None
            shell.console.print("[yellow]Previously selected device is gone[/yellow]")

    if not shell.adb.selected_device and len(online) == 1:
        shell.adb.selected_device = online[0]
        shell.console.print(f"[green]Auto-selected: {online[0].display_name}[/green]")

    # Refresh PID if we were tracking a package
    pkg = shell.log_stream.filters.package
    if pkg and shell.adb.selected_device:
        pid = shell.adb.get_pid(pkg)
        if pid:
            shell.log_stream.filters.update_pid(pid)
            shell.console.print(f"[green]PID for {pkg}: {pid}[/green]")

    if shell.adb.selected_device:
        await _auto_start_stream(shell)


# --- Logs ---

async def cmd_app(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /app <package.name>[/red]")
        return

    if not shell.adb.selected_device:
        dev = shell.adb.auto_select()
        if not dev:
            shell.console.print("[red]No device selected. Use /devices then /connect[/red]")
            return
        shell.console.print(f"[green]Auto-selected: {dev.display_name}[/green]")

    # Save current filters under previous package before switching
    prev_pkg = shell.log_stream.filters.package
    if prev_pkg and prev_pkg != args:
        save_app_filters(prev_pkg, shell.log_stream.filters)

    pid = shell.adb.get_pid(args)
    shell.log_stream.filters.set_package(args, pid)

    # Auto-restore prior filter state for this package, if any
    restored = load_app_filters(args, shell.log_stream.filters)

    if pid:
        shell.console.print(f"[green]Tracking app: {args} (PID: {pid})[/green]")
    else:
        shell.console.print(f"[yellow]App {args} not running — will track when started[/yellow]")

    if restored:
        shell.console.print(f"[dim]Restored saved filters for {args}[/dim]")

    push_app_history(args)
    await _auto_start_stream(shell)


async def cmd_pid(shell: LoguxShell, args: str) -> None:
    if not args or not args.isdigit():
        shell.console.print("[red]Usage: /pid <number>[/red]")
        return
    shell.log_stream.filters.set_pid(int(args))
    shell.console.print(f"[green]Filter: PID = {args}[/green]")
    _autosave_filters(shell)
    await _auto_start_stream(shell)


async def cmd_tag(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /tag <tag> | /tag -<tag> | /tag reset[/red]")
        return
    if args == "reset":
        shell.log_stream.filters.reset_tags()
        shell.console.print("[green]Tag filters cleared[/green]")
    elif args.startswith("-"):
        tag = args[1:].strip()
        if tag:
            shell.log_stream.filters.remove_tag(tag)
            shell.console.print(f"[yellow]Removed tag: {tag}[/yellow]")
    else:
        shell.log_stream.filters.add_tag(args)
        shell.console.print(f"[green]Filter: added tag '{args}'[/green]")
    _autosave_filters(shell)
    await _auto_start_stream(shell)


async def cmd_level(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /level <V|D|I|W|E|F> | /level reset[/red]")
        return
    if args == "reset":
        shell.log_stream.filters.reset_level()
        shell.console.print("[green]Level filter cleared[/green]")
        _autosave_filters(shell)
        return
    level_char = args[0].upper()
    try:
        level = LogLevel.from_char(level_char)
    except (KeyError, IndexError):
        shell.console.print(f"[red]Unknown level: {args}. Use V, D, I, W, E, or F[/red]")
        return
    shell.log_stream.filters.set_level(level)
    shell.console.print(f"[green]Filter: level >= {level.name}[/green]")
    _autosave_filters(shell)


async def cmd_grep(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /grep <text> | /grep reset[/red]")
        return
    if args == "reset":
        shell.log_stream.filters.reset_text()
        shell.log_stream.formatter.highlight_text = ""
        shell.console.print("[green]Grep filter cleared[/green]")
        _autosave_filters(shell)
        return
    shell.log_stream.filters.set_text(args)
    shell.log_stream.formatter.highlight_text = args
    shell.console.print(f"[green]Filter: text contains '{args}'[/green]")
    _autosave_filters(shell)


async def cmd_msg(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /msg <text> | /msg -<text> | /msg reset[/red]")
        return
    if args == "reset":
        shell.log_stream.filters.reset_msgs()
        shell.console.print("[green]Msg filters cleared[/green]")
    elif args.startswith("-"):
        text = args[1:].strip()
        if text:
            shell.log_stream.filters.remove_msg(text)
            shell.console.print(f"[yellow]Removed msg: {text}[/yellow]")
    else:
        shell.log_stream.filters.add_msg(args)
        shell.console.print(f"[green]Filter: msg contains '{args}' (OR)[/green]")
    _autosave_filters(shell)


async def cmd_regex(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /regex <pattern> | /regex reset[/red]")
        return
    if args == "reset":
        shell.log_stream.filters.reset_regex()
        shell.console.print("[green]Regex filter cleared[/green]")
        _autosave_filters(shell)
        return
    try:
        shell.log_stream.filters.set_regex(args)
        shell.console.print(f"[green]Filter: regex '{args}'[/green]")
        _autosave_filters(shell)
    except Exception as exc:
        shell.console.print(f"[red]Invalid regex: {exc}[/red]")


async def cmd_exclude(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print("[red]Usage: /exclude tag|msg <value> | show | reset | remove <value>[/red]")
        return
    parts = args.split(maxsplit=1)
    sub = parts[0].lower()
    rest = parts[1].strip() if len(parts) > 1 else ""
    filters = shell.log_stream.filters

    if sub == "show":
        if not filters.exclude_tags and not filters.exclude_msgs:
            shell.console.print("[dim]No exclusions[/dim]")
            return
        if filters.exclude_tags:
            shell.console.print("[yellow]!tag[/yellow] " + ", ".join(sorted(filters.exclude_tags)))
        if filters.exclude_msgs:
            shell.console.print("[yellow]!msg[/yellow] " + " | ".join(sorted(filters.exclude_msgs)))
    elif sub == "reset":
        filters.reset_excludes()
        shell.console.print("[green]Exclusions cleared[/green]")
        _autosave_filters(shell)
    elif sub == "remove":
        if not rest:
            shell.console.print("[red]Usage: /exclude remove <value>[/red]")
            return
        if filters.remove_exclude(rest):
            shell.console.print(f"[yellow]Removed exclusion: {rest}[/yellow]")
            _autosave_filters(shell)
        else:
            shell.console.print(f"[red]Not found in exclusions: {rest}[/red]")
    elif sub == "tag":
        if not rest:
            shell.console.print("[red]Usage: /exclude tag <tag>[/red]")
            return
        filters.add_exclude_tag(rest)
        shell.console.print(f"[green]Hiding tag: {rest}[/green]")
        _autosave_filters(shell)
    elif sub == "msg":
        if not rest:
            shell.console.print("[red]Usage: /exclude msg <text>[/red]")
            return
        filters.add_exclude_msg(rest)
        shell.console.print(f"[green]Hiding messages with: {rest}[/green]")
        _autosave_filters(shell)
    else:
        shell.console.print("[red]Usage: /exclude tag|msg <value> | show | reset | remove <value>[/red]")


async def cmd_filter(shell: LoguxShell, args: str) -> None:
    filters = shell.log_stream.filters
    parts = args.split(maxsplit=1)
    sub = parts[0].lower() if parts else ""
    rest = parts[1] if len(parts) > 1 else ""

    if sub == "reset":
        filters.reset()
        shell.log_stream.formatter.highlight_text = ""
        shell.console.print("[green]All filters cleared[/green]")
        _autosave_filters(shell)
    elif sub == "show":
        shell.console.print(f"[cyan]Active filters: {filters.description}[/cyan]")
    elif sub == "edit":
        expr = filters.to_edit_string()
        shell.console.print(f"[dim]/filter set {expr}[/dim]" if expr else "[dim](no filters — nothing to edit)[/dim]")
    elif sub == "set":
        if not rest:
            shell.console.print("[red]Usage: /filter set app=… tag=a,b level=W grep=… msg=… !tag=… !msg=…[/red]")
            return
        filters.apply_edit_string(rest)
        shell.log_stream.formatter.highlight_text = filters.text
        shell.console.print(f"[green]Filters applied: {filters.description}[/green]")
        save_filter_preset(rest)
        _autosave_filters(shell)
    else:
        shell.console.print("[red]Usage: /filter show | reset | edit | set <expr>[/red]")


# --- Format ---

async def cmd_format(shell: LoguxShell, args: str) -> None:
    if not args:
        shell.console.print(f"[cyan]Current: {shell.log_stream.formatter.config.preset.value}[/cyan]")
        shell.console.print("[dim]Available: compact, threadtime, verbose, minimal, json[/dim]")
        return
    try:
        preset = Preset(args.lower())
        shell.log_stream.formatter.config.apply_preset(preset)
        shell.console.print(f"[green]Format: {preset.value}[/green]")
    except ValueError:
        shell.console.print(f"[red]Unknown preset: {args}. Use compact, threadtime, verbose, minimal, json[/red]")


async def cmd_fields(shell: LoguxShell, args: str) -> None:
    if not args:
        cfg = shell.log_stream.formatter.config
        fields = []
        for name in ("timestamp", "level", "tag", "pid", "tid", "message"):
            val = getattr(cfg, name)
            fields.append(f"[green]+{name}[/green]" if val else f"[red]-{name}[/red]")
        shell.console.print("Fields: " + " ".join(fields))
        return

    for token in args.split():
        if len(token) < 2 or token[0] not in ("+", "-"):
            shell.console.print(f"[red]Invalid: {token} (use +field or -field)[/red]")
            continue
        enabled = token[0] == "+"
        field_name = token[1:]
        if shell.log_stream.formatter.config.toggle_field(field_name, enabled):
            state = "on" if enabled else "off"
            shell.console.print(f"[green]{field_name}: {state}[/green]")
        else:
            shell.console.print(f"[red]Unknown field: {field_name}[/red]")


# --- Control ---

async def cmd_pause(shell: LoguxShell, args: str) -> None:
    paused = shell.log_stream.toggle_pause()
    if paused:
        shell.console.print("[yellow]⏸ Paused — logs still captured, /resume or /pause to continue[/yellow]")
    else:
        shell.console.print("[green]▶ Resumed[/green]")


async def cmd_resume(shell: LoguxShell, args: str) -> None:
    shell.log_stream.resume()
    shell.console.print("[green]▶ Resumed[/green]")


async def cmd_stop(shell: LoguxShell, args: str) -> None:
    if shell.log_stream.is_running:
        await shell.log_stream.stop()
        shell.console.print("[yellow]Log stream stopped (no auto-reconnect)[/yellow]")
    else:
        shell.console.print("[dim]Log stream is not running[/dim]")


async def cmd_save(shell: LoguxShell, args: str) -> None:
    if not args:
        current = shell.log_stream.save_path
        if current:
            shell.console.print(f"[cyan]Saving to: {current}[/cyan] — /save off to stop")
        else:
            shell.console.print("[red]Usage: /save <filename> | /save off[/red]")
        return
    if args.lower() == "off":
        shell.log_stream.stop_save()
        shell.console.print("[yellow]Save stopped[/yellow]")
        return
    shell.log_stream.start_save(args)
    shell.console.print(f"[green]Saving matching logs to: {args}[/green]")


async def cmd_copy(shell: LoguxShell, args: str) -> None:
    n = 100
    if args.strip().isdigit():
        n = max(1, int(args.strip()))
    messages = shell.log_stream.recent_messages
    if not messages:
        shell.console.print("[dim]No messages captured yet[/dim]")
        return
    slice_ = messages[-n:]
    text = "\n".join(slice_)

    try:
        import pyperclip  # type: ignore
        pyperclip.copy(text)
        shell.console.print(f"[green]Copied {len(slice_)} messages to clipboard[/green]")
    except Exception as exc:
        shell.console.print(f"[yellow]Clipboard unavailable ({exc}) — printing instead[/yellow]")
        shell.console.print(text)


# --- Presets ---

async def cmd_preset(shell: LoguxShell, args: str) -> None:
    parts = args.split(maxsplit=1)
    if not parts:
        shell.console.print("[red]Usage: /preset save|load|list|delete <name>[/red]")
        return

    sub = parts[0].lower()
    name = parts[1] if len(parts) > 1 else ""

    if sub == "save":
        if not name:
            shell.console.print("[red]Usage: /preset save <name>[/red]")
            return
        path = save_preset(name, shell.log_stream.filters, shell.log_stream.formatter.config)
        shell.console.print(f"[green]Preset saved: {name} → {path}[/green]")

    elif sub == "load":
        if not name:
            shell.console.print("[red]Usage: /preset load <name>[/red]")
            return
        if load_preset(name, shell.log_stream.filters, shell.log_stream.formatter.config):
            shell.log_stream.formatter.highlight_text = shell.log_stream.filters.text
            shell.console.print(f"[green]Preset loaded: {name}[/green]")
        else:
            shell.console.print(f"[red]Preset not found: {name}[/red]")

    elif sub == "list":
        presets = list_presets()
        filter_presets = list_filter_presets()
        if presets:
            shell.console.print("[cyan]Named presets:[/cyan] " + ", ".join(presets))
        else:
            shell.console.print("[dim]No named presets[/dim]")
        if filter_presets:
            shell.console.print("[cyan]Auto-saved filter expressions:[/cyan]")
            for name_, expr in filter_presets[-10:]:
                shell.console.print(f"  [dim]{name_}[/dim]  {expr}")

    elif sub == "delete":
        if not name:
            shell.console.print("[red]Usage: /preset delete <name>[/red]")
            return
        if delete_preset(name):
            shell.console.print(f"[green]Deleted: {name}[/green]")
        else:
            shell.console.print(f"[red]Not found: {name}[/red]")
    else:
        shell.console.print("[red]Usage: /preset save|load|list|delete <name>[/red]")


async def cmd_forget(shell: LoguxShell, args: str) -> None:
    p, a, h = clear_saved_filters()
    shell.console.print(
        f"[green]Forgotten: {p} filter presets, {a} per-app states, {h} history entries[/green]"
    )


# --- Traffic ---

async def cmd_traffic(shell: LoguxShell, args: str) -> None:
    parts = args.split(maxsplit=1)
    if not parts:
        shell.console.print("[red]Usage: /traffic open|close|list|inspect|filter|clear[/red]")
        return

    sub = parts[0].lower()
    rest = parts[1] if len(parts) > 1 else ""

    if sub == "open":
        ok, msg = shell.traffic.start()
        style = "green" if ok else "red"
        shell.console.print(f"[{style}]{msg}[/{style}]")
        if ok:
            shell.console.print(
                f"[dim]Configure device proxy: {shell.traffic.listen_port}[/dim]"
            )

    elif sub == "close":
        ok, msg = shell.traffic.stop()
        shell.console.print(f"[yellow]{msg}[/yellow]")

    elif sub == "list":
        entries = shell.traffic.get_entries()
        if not entries:
            shell.console.print("[dim]No traffic captured[/dim]")
            return
        table = Table(show_header=True, header_style="bold")
        table.add_column("#", style="dim", width=5)
        table.add_column("Time", width=12)
        table.add_column("Method", width=7)
        table.add_column("Status", width=6)
        table.add_column("Host")
        table.add_column("Path")
        for e in entries:
            status_style = "green" if e.status and e.status < 400 else "red" if e.status and e.status >= 400 else "dim"
            table.add_row(
                str(e.id),
                e.timestamp,
                e.method,
                Text(str(e.status or "..."), style=status_style),
                e.host,
                e.path,
            )
        shell.console.print(table)

    elif sub == "inspect":
        if not rest or not rest.isdigit():
            shell.console.print("[red]Usage: /traffic inspect <id>[/red]")
            return
        entry = shell.traffic.get_entry(int(rest))
        if not entry:
            shell.console.print(f"[red]Entry #{rest} not found[/red]")
            return

        shell.console.print(Panel(
            f"[bold]{entry.method}[/bold] {entry.url}\n"
            f"Status: {entry.status or 'pending'}\n"
            f"Time: {entry.timestamp}\n\n"
            f"[bold]Request Headers:[/bold]\n"
            + "\n".join(f"  {k}: {v}" for k, v in entry.request_headers.items())
            + "\n\n[bold]Request Body:[/bold]\n"
            + (entry.request_body.decode("utf-8", errors="replace")[:2000] or "(empty)")
            + "\n\n[bold]Response Headers:[/bold]\n"
            + "\n".join(f"  {k}: {v}" for k, v in entry.response_headers.items())
            + "\n\n[bold]Response Body:[/bold]\n"
            + (entry.response_body.decode("utf-8", errors="replace")[:2000] or "(empty)"),
            title=f"Traffic #{entry.id}",
        ))

    elif sub == "filter":
        if not rest:
            shell.traffic.filter.reset()
            shell.console.print("[green]Traffic filter cleared[/green]")
            return
        for pair in rest.split():
            if "=" in pair:
                k, v = pair.split("=", 1)
                k = k.lower()
                if k == "host":
                    shell.traffic.filter.host = v
                elif k == "path":
                    shell.traffic.filter.path = v
                elif k == "method":
                    shell.traffic.filter.method = v
                elif k == "status":
                    shell.traffic.filter.status = int(v) if v.isdigit() else None
                elif k == "body":
                    shell.traffic.filter.body_search = v
        shell.console.print("[green]Traffic filter updated[/green]")

    elif sub == "clear":
        shell.traffic.clear()
        shell.console.print("[green]Traffic cleared[/green]")

    else:
        shell.console.print("[red]Usage: /traffic open|close|list|inspect|filter|clear[/red]")


# --- Mock ---

async def cmd_mock(shell: LoguxShell, args: str) -> None:
    parts = args.split(maxsplit=1)
    if not parts:
        shell.console.print("[red]Usage: /mock load|list|enable|disable|reload[/red]")
        return

    sub = parts[0].lower()
    rest = parts[1] if len(parts) > 1 else ""

    if sub == "load":
        if not rest:
            shell.console.print("[red]Usage: /mock load <rules.yaml>[/red]")
            return
        ok, msg = shell.mock_engine.load(rest)
        style = "green" if ok else "red"
        shell.console.print(f"[{style}]{msg}[/{style}]")
        if ok:
            shell.traffic.set_mock_handler(shell.mock_engine)

    elif sub == "list":
        if not shell.mock_engine.rules:
            shell.console.print("[dim]No rules loaded[/dim]")
            return
        table = Table(show_header=True, header_style="bold")
        table.add_column("ID", style="cyan")
        table.add_column("Enabled")
        table.add_column("Match")
        table.add_column("Response")
        table.add_column("Hits", justify="right")
        for rule in shell.mock_engine.rules:
            enabled = Text("ON", style="green") if rule.enabled else Text("OFF", style="red")
            match_desc = f"{rule.match.method or '*'} {rule.match.path or '*'}"
            resp_desc = f"{rule.response.type} → {rule.response.status}"
            table.add_row(rule.id, enabled, match_desc, resp_desc, str(rule.hit_count))
        shell.console.print(table)

    elif sub == "enable":
        if not rest:
            shell.console.print("[red]Usage: /mock enable <rule_id>[/red]")
            return
        if shell.mock_engine.enable_rule(rest):
            shell.console.print(f"[green]Enabled: {rest}[/green]")
        else:
            shell.console.print(f"[red]Rule not found: {rest}[/red]")

    elif sub == "disable":
        if not rest:
            shell.console.print("[red]Usage: /mock disable <rule_id>[/red]")
            return
        if shell.mock_engine.disable_rule(rest):
            shell.console.print(f"[yellow]Disabled: {rest}[/yellow]")
        else:
            shell.console.print(f"[red]Rule not found: {rest}[/red]")

    elif sub == "reload":
        ok, msg = shell.mock_engine.reload()
        style = "green" if ok else "red"
        shell.console.print(f"[{style}]{msg}[/{style}]")

    else:
        shell.console.print("[red]Usage: /mock load|list|enable|disable|reload[/red]")


# --- Helpers ---

async def _auto_start_stream(shell: LoguxShell) -> None:
    """Auto-start log stream if not already running and device is available."""
    if shell.log_stream.is_running:
        return
    if not shell.adb.selected_device:
        dev = shell.adb.auto_select()
        if not dev:
            return
    await shell.log_stream.start()


def _autosave_filters(shell: LoguxShell) -> None:
    """Persist current filters under the active package, if any."""
    pkg = shell.log_stream.filters.package
    if pkg:
        save_app_filters(pkg, shell.log_stream.filters)
