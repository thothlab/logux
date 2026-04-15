//! Command dispatcher — handles all /commands from the shell.

use crate::adb::AdbClient;
use crate::config;
use crate::logs::filters::FilterState;
use crate::logs::formatter::{LogFormatter, Preset};
use crate::logs::parser::LogLevel;
use crate::mock::MockEngine;
use crate::traffic::TrafficProxy;

/// Context passed to every command handler.
pub struct CommandContext<'a> {
    pub adb: &'a mut AdbClient,
    pub filters: &'a mut FilterState,
    pub formatter: &'a mut LogFormatter,
    pub traffic: &'a mut TrafficProxy,
    pub mock_engine: &'a mut MockEngine,
    pub streaming: &'a mut bool,
    pub paused: &'a mut bool,
    pub save_path: &'a mut Option<String>,
    pub exit_requested: &'a mut bool,
    pub output: &'a mut Vec<String>,
}

pub async fn dispatch(ctx: &mut CommandContext<'_>, input: &str) {
    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "/help" => cmd_help(ctx),
        "/exit" | "/quit" | "/q" => *ctx.exit_requested = true,
        "/clear" => {} // handled by TUI directly
        "/devices" => cmd_devices(ctx),
        "/connect" => cmd_connect(ctx, args),
        "/disconnect" => cmd_disconnect(ctx),
        "/app" => cmd_app(ctx, args),
        "/pid" => cmd_pid(ctx, args),
        "/tag" => cmd_tag(ctx, args),
        "/level" => cmd_level(ctx, args),
        "/grep" => cmd_grep(ctx, args),
        "/msg" => cmd_msg(ctx, args),
        "/regex" => cmd_regex(ctx, args),
        "/filter" => cmd_filter(ctx, args),
        "/exclude" => cmd_exclude(ctx, args),
        "/format" => cmd_format(ctx, args),
        "/fields" => cmd_fields(ctx, args),
        "/stop" => cmd_stop(ctx),
        "/pause" => cmd_pause(ctx),
        "/resume" => cmd_resume(ctx),
        "/save" => cmd_save(ctx, args),
        "/preset" => cmd_preset(ctx, args),
        "/traffic" => cmd_traffic(ctx, args).await,
        "/mock" => cmd_mock(ctx, args),
        _ => ctx.output.push(format!("\x1b[31mUnknown command: {cmd}\x1b[0m — type /help")),
    }
}

fn cmd_help(ctx: &mut CommandContext) {
    ctx.output.push(format!("\x1b[1;36m{:<30} {}\x1b[0m", "Command", "Description"));
    ctx.output.push("─".repeat(60));

    let commands = [
        ("/help", "Show this help"),
        ("/exit", "Exit logux"),
        ("/clear", "Clear screen"),
        ("", ""),
        ("--- ADB ---", ""),
        ("/devices", "List connected devices"),
        ("/connect <ip:port>", "Connect to TCP device"),
        ("/disconnect", "Disconnect current device"),
        ("", ""),
        ("--- Logs ---", ""),
        ("/app <package>", "Filter by app (smart PID tracking)"),
        ("/pid <pid>", "Filter by PID"),
        ("/tag <tag>", "Add tag filter (-tag remove, reset clear)"),
        ("/level <V|D|I|W|E|F>", "Min log level (reset to clear)"),
        ("/grep <text>", "Filter by text in tag+message (reset to clear)"),
        ("/msg <text>", "Filter by text in message only (-text remove, reset clear)"),
        ("/regex <pattern>", "Filter by regex (reset to clear)"),
        ("/filter reset|show|<preset>", "Clear, show, or load preset"),
        ("/exclude tag|msg <value>", "Exclude by tag/message"),
        ("/exclude show|reset|remove", "Manage exclusions"),
        ("", ""),
        ("--- Format ---", ""),
        ("/format <preset>", "compact|threadtime|verbose|minimal|json"),
        ("/fields +field -field", "Toggle: timestamp,level,tag,pid,tid"),
        ("", ""),
        ("--- Control ---", ""),
        ("/stop", "Stop log stream completely"),
        ("/pause", "Toggle pause (logs captured but hidden)"),
        ("/resume", "Resume after pause"),
        ("/save <file>", "Save matching logs to file"),
        ("", ""),
        ("--- Presets ---", ""),
        ("/preset save <name>", "Save current config"),
        ("/preset load <name>", "Load saved config"),
        ("/preset list", "List saved presets"),
        ("/preset delete <name>", "Delete a preset"),
        ("", ""),
        ("--- Traffic ---", ""),
        ("/traffic open", "Start proxy"),
        ("/traffic close", "Stop proxy"),
        ("/traffic list", "Show requests"),
        ("/traffic inspect <id>", "Inspect request/response"),
        ("/traffic filter <expr>", "Filter: host=,path=,method=,status="),
        ("/traffic clear", "Clear captured traffic"),
        ("", ""),
        ("--- Mock ---", ""),
        ("/mock load <file.yaml>", "Load mock rules"),
        ("/mock list", "List rules"),
        ("/mock enable <id>", "Enable a rule"),
        ("/mock disable <id>", "Disable a rule"),
        ("/mock reload", "Reload rules from file"),
        ("", ""),
        ("--- Keys ---", ""),
        ("PageUp / PageDown", "Scroll logs"),
        ("Tab", "Auto-complete"),
        ("Ctrl+C", "Exit"),
        ("Ctrl+L", "Clear log"),
    ];

    for (cmd, desc) in commands {
        if cmd.is_empty() {
            ctx.output.push(String::new());
        } else if cmd.starts_with("---") {
            ctx.output.push(format!("\x1b[1m{cmd}\x1b[0m"));
        } else {
            ctx.output.push(format!("  \x1b[32m{:<28}\x1b[0m {}", cmd, desc));
        }
    }
}

fn cmd_devices(ctx: &mut CommandContext) {
    let devices = ctx.adb.list_devices().to_vec();
    if devices.is_empty() {
        ctx.output.push("\x1b[33mNo devices found\x1b[0m".to_string());
        return;
    }
    let selected_serial = ctx.adb.selected_device.as_ref().map(|d| d.serial.clone());
    ctx.output.push(format!(
        "\x1b[1m{:<24} {:<14} {:<12} {:<6} {}\x1b[0m",
        "Serial", "State", "Model", "Type", ""
    ));
    for dev in &devices {
        let state_color = if dev.is_online() { "32" } else { "31" };
        let selected = if selected_serial.as_deref() == Some(&dev.serial) { " <-" } else { "" };
        let conn = match dev.connection_type() {
            crate::adb::ConnectionType::Usb => "USB",
            crate::adb::ConnectionType::Tcp => "TCP",
        };
        let model = if dev.model.is_empty() { &dev.product } else { &dev.model };
        ctx.output.push(format!(
            "  {:<24} \x1b[{state_color}m{:<14}\x1b[0m {:<12} {:<6} {}",
            dev.serial,
            dev.state.as_str(),
            model,
            conn,
            selected,
        ));
    }
}

fn cmd_connect(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        ctx.output.push("\x1b[31mUsage: /connect <ip:port>\x1b[0m".to_string());
        return;
    }
    let (ok, msg) = ctx.adb.connect_tcp(args);
    let color = if ok { "32" } else { "31" };
    ctx.output.push(format!("\x1b[{color}m{msg}\x1b[0m"));
    if ok {
        *ctx.streaming = true;
    }
}

fn cmd_disconnect(ctx: &mut CommandContext) {
    let (_, msg) = ctx.adb.disconnect(None);
    ctx.output.push(format!("\x1b[33m{msg}\x1b[0m"));
    *ctx.streaming = false;
}

fn cmd_app(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        ctx.output.push("\x1b[31mUsage: /app <package.name>\x1b[0m".to_string());
        return;
    }
    if ctx.adb.selected_device.is_none() {
        if ctx.adb.auto_select().is_none() {
            ctx.output.push("\x1b[31mNo device selected. Use /devices then /connect\x1b[0m".to_string());
            return;
        }
        let name = ctx.adb.selected_device.as_ref().unwrap().display_name();
        ctx.output.push(format!("\x1b[32mAuto-selected: {name}\x1b[0m"));
    }
    let pid = ctx.adb.get_pid(args);
    ctx.filters.set_package(args, pid);
    if let Some(p) = pid {
        ctx.output.push(format!("\x1b[32mTracking app: {args} (PID: {p})\x1b[0m"));
    } else {
        ctx.output.push(format!("\x1b[33mApp {args} not running — will track when started\x1b[0m"));
    }

    // Auto-restore last used filters for this app
    if let Some(saved) = config::load_app_filters(args) {
        // Preserve the package/pid we just set
        let pkg = ctx.filters.package.clone();
        let pids = ctx.filters.pids.clone();
        let tracking = ctx.filters.package_tracking;
        ctx.filters.apply_edit_string(&saved);
        ctx.filters.package = pkg;
        ctx.filters.pids = pids;
        ctx.filters.package_tracking = tracking;
        ctx.output.push(format!("\x1b[36mRestored filters: {saved}\x1b[0m"));
    }

    *ctx.streaming = true;
}

fn cmd_pid(ctx: &mut CommandContext, args: &str) {
    match args.parse::<u32>() {
        Ok(pid) => {
            ctx.filters.set_pid(pid);
            ctx.output.push(format!("\x1b[32mFilter: PID = {pid}\x1b[0m"));
            *ctx.streaming = true;
        }
        Err(_) => ctx.output.push("\x1b[31mUsage: /pid <number>\x1b[0m".to_string()),
    }
}

fn cmd_tag(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        if ctx.filters.tags.is_empty() {
            ctx.output.push("\x1b[2mNo tag filters\x1b[0m".to_string());
        } else {
            let tags: Vec<_> = ctx.filters.tags.iter().cloned().collect();
            ctx.output.push(format!("\x1b[36mTag filters: {}\x1b[0m", tags.join(", ")));
        }
        ctx.output.push("\x1b[2mUsage: /tag <name> to add, /tag -<name> to remove, /tag reset to clear\x1b[0m".to_string());
        return;
    }
    if args == "reset" {
        ctx.filters.clear_tags();
        ctx.output.push("\x1b[32mTag filters cleared\x1b[0m".to_string());
    } else if let Some(tag) = args.strip_prefix('-') {
        ctx.filters.remove_tag(tag);
        ctx.output.push(format!("\x1b[33mRemoved tag filter: '{tag}'\x1b[0m"));
    } else {
        ctx.filters.add_tag(args);
        ctx.output.push(format!("\x1b[32mFilter: added tag '{args}'\x1b[0m"));
    }
    *ctx.streaming = true;
}

fn cmd_level(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() || args == "reset" {
        ctx.filters.clear_level();
        ctx.output.push("\x1b[32mLevel filter cleared (showing all levels)\x1b[0m".to_string());
        return;
    }
    match LogLevel::from_name(args) {
        Some(level) => {
            ctx.filters.set_level(level);
            ctx.output.push(format!("\x1b[32mFilter: level >= {}\x1b[0m", level.char()));
        }
        None => ctx.output.push(format!("\x1b[31mUnknown level: {args}. Use V, D, I, W, E, or F (or 'reset')\x1b[0m")),
    }
}

fn cmd_grep(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() || args == "reset" {
        ctx.filters.clear_text();
        ctx.formatter.highlight_text.clear();
        ctx.output.push("\x1b[32mText filter cleared\x1b[0m".to_string());
        return;
    }
    ctx.filters.set_text(args);
    ctx.formatter.highlight_text = args.to_string();
    ctx.output.push(format!("\x1b[32mFilter: text contains '{args}'\x1b[0m"));
}

fn cmd_msg(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        if ctx.filters.messages.is_empty() {
            ctx.output.push("\x1b[2mNo message filters\x1b[0m".to_string());
        } else {
            let joined = ctx.filters.messages.join(", ");
            ctx.output.push(format!("\x1b[36mMessage filters: {joined}\x1b[0m"));
        }
        ctx.output.push("\x1b[2mUsage: /msg <text> to add, /msg -<text> to remove, /msg reset to clear\x1b[0m".to_string());
        return;
    }
    if args == "reset" {
        ctx.filters.clear_messages();
        ctx.output.push("\x1b[32mMessage filters cleared\x1b[0m".to_string());
    } else if let Some(m) = args.strip_prefix('-') {
        ctx.filters.remove_message(m);
        ctx.output.push(format!("\x1b[32mRemoved message filter: {m}\x1b[0m"));
    } else {
        ctx.filters.add_message(args);
        ctx.output.push(format!("\x1b[32mFilter: message contains '{args}'\x1b[0m"));
    }
}

fn cmd_regex(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() || args == "reset" {
        ctx.filters.clear_regex();
        ctx.output.push("\x1b[32mRegex filter cleared\x1b[0m".to_string());
        return;
    }
    match ctx.filters.set_regex(args) {
        Ok(()) => ctx.output.push(format!("\x1b[32mFilter: regex '{args}'\x1b[0m")),
        Err(e) => ctx.output.push(format!("\x1b[31mInvalid regex: {e}\x1b[0m")),
    }
}

fn cmd_filter(ctx: &mut CommandContext, args: &str) {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    let sub = parts[0];
    let value = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match sub {
        "reset" => {
            ctx.filters.reset();
            ctx.formatter.highlight_text.clear();
            ctx.output.push("\x1b[32mAll filters cleared\x1b[0m".to_string());
        }
        "show" => {
            ctx.output.push(format!("\x1b[36mActive filters: {}\x1b[0m", ctx.filters.description()));
        }
        "edit" => {
            // Handled by TUI — should not reach here
        }
        "set" => {
            if value.is_empty() {
                ctx.output.push("\x1b[31mUsage: /filter set app=com.pkg tag=X level=W !tag=Y\x1b[0m".to_string());
                return;
            }
            // Strip trailing "# comment" from preset suggestions
            let clean_value = if let Some(pos) = value.find("  #") {
                value[..pos].trim()
            } else {
                value
            };
            ctx.filters.apply_edit_string(clean_value);
            // Sync highlight
            if ctx.filters.text.is_empty() {
                ctx.formatter.highlight_text.clear();
            } else {
                ctx.formatter.highlight_text = ctx.filters.text.clone();
            }
            // Auto-save filter preset
            let edit_str = ctx.filters.to_edit_string();
            config::save_filter_preset(&edit_str);
            ctx.output.push(format!("\x1b[32mFilters updated: {}\x1b[0m", ctx.filters.description()));
            *ctx.streaming = true;
        }
        "tag" => {
            cmd_tag(ctx, value);
            return;
        }
        "level" => {
            cmd_level(ctx, value);
            return;
        }
        "grep" | "text" => {
            cmd_grep(ctx, value);
            return;
        }
        "msg" | "message" => {
            cmd_msg(ctx, value);
            return;
        }
        "regex" => {
            cmd_regex(ctx, value);
            return;
        }
        "exclude" => {
            cmd_exclude(ctx, value);
            return;
        }
        "app" => {
            cmd_app(ctx, value);
            return;
        }
        "" => {
            ctx.output.push("\x1b[31mUsage: /filter tag|level|grep|msg|regex|exclude|reset|show|<preset>\x1b[0m".to_string());
        }
        preset_name => {
            // Try to load a preset
            match config::load_preset(preset_name, ctx.filters, &mut ctx.formatter.config) {
                Ok(()) => {
                    ctx.output.push(format!("\x1b[32mFilter preset loaded: {preset_name}\x1b[0m"));
                    // Save to filter history for current app
                    if !ctx.filters.package.is_empty() {
                        config::save_filter_to_history(&ctx.filters.package, preset_name);
                    }
                    *ctx.streaming = true;
                }
                Err(e) => ctx.output.push(format!("\x1b[31m{e}\x1b[0m")),
            }
        }
    }
}

fn cmd_exclude(ctx: &mut CommandContext, args: &str) {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    let sub = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let value = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match sub.as_str() {
        "tag" => {
            if value.is_empty() {
                ctx.output.push("\x1b[31mUsage: /exclude tag <name>\x1b[0m".to_string());
                return;
            }
            ctx.filters.exclude_tags.insert(value.to_string());
            ctx.output.push(format!("\x1b[33mExcluding tag: '{value}'\x1b[0m"));
        }
        "msg" | "message" | "text" => {
            if value.is_empty() {
                ctx.output.push("\x1b[31mUsage: /exclude msg <text>\x1b[0m".to_string());
                return;
            }
            ctx.filters.exclude_texts.push(value.to_string());
            ctx.output.push(format!("\x1b[33mExcluding messages containing: '{value}'\x1b[0m"));
        }
        "show" => {
            if ctx.filters.exclude_tags.is_empty() && ctx.filters.exclude_texts.is_empty() {
                ctx.output.push("\x1b[2mNo exclusion filters\x1b[0m".to_string());
            } else {
                ctx.output.push("\x1b[36mExclusion filters (none of these pass):\x1b[0m".to_string());
                for tag in &ctx.filters.exclude_tags {
                    ctx.output.push(format!("  \x1b[33mtag contains '{tag}'\x1b[0m"));
                }
                for text in &ctx.filters.exclude_texts {
                    ctx.output.push(format!("  \x1b[33mmsg contains '{text}'\x1b[0m"));
                }
            }
            return;
        }
        "reset" => {
            ctx.filters.exclude_tags.clear();
            ctx.filters.exclude_texts.clear();
            ctx.output.push("\x1b[32mAll exclusion filters cleared\x1b[0m".to_string());
        }
        "remove" => {
            if value.is_empty() {
                ctx.output.push("\x1b[31mUsage: /exclude remove <tag_or_text>\x1b[0m".to_string());
                return;
            }
            let removed_tag = ctx.filters.exclude_tags.remove(value);
            let removed_text = ctx.filters.exclude_texts.iter().position(|t| t == value);
            if let Some(pos) = removed_text {
                ctx.filters.exclude_texts.remove(pos);
                ctx.output.push(format!("\x1b[32mRemoved exclusion: '{value}'\x1b[0m"));
            } else if removed_tag {
                ctx.output.push(format!("\x1b[32mRemoved exclusion: '{value}'\x1b[0m"));
            } else {
                ctx.output.push(format!("\x1b[31mExclusion not found: '{value}'\x1b[0m"));
            }
        }
        _ => {
            ctx.output.push("\x1b[31mUsage: /exclude tag|msg|show|reset|remove\x1b[0m".to_string());
            ctx.output.push("\x1b[2m  /exclude tag System.out     -- hide lines with this tag\x1b[0m".to_string());
            ctx.output.push("\x1b[2m  /exclude msg \"[socket]:\"    -- hide lines containing text\x1b[0m".to_string());
            ctx.output.push("\x1b[2m  /exclude show               -- list exclusions\x1b[0m".to_string());
            ctx.output.push("\x1b[2m  /exclude reset              -- clear all exclusions\x1b[0m".to_string());
            ctx.output.push("\x1b[2m  /exclude remove <value>     -- remove one exclusion\x1b[0m".to_string());
        }
    }
}

fn cmd_format(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        ctx.output.push(format!("\x1b[36mCurrent: {}\x1b[0m", ctx.formatter.config.preset.as_str()));
        ctx.output.push("\x1b[2mAvailable: compact, threadtime, verbose, minimal, json\x1b[0m".to_string());
        return;
    }
    match Preset::from_name(args) {
        Some(preset) => {
            ctx.formatter.config.apply_preset(preset);
            ctx.output.push(format!("\x1b[32mFormat: {}\x1b[0m", preset.as_str()));
        }
        None => ctx.output.push(format!("\x1b[31mUnknown preset: {args}\x1b[0m")),
    }
}

fn cmd_fields(ctx: &mut CommandContext, args: &str) {
    if args.is_empty() {
        let cfg = &ctx.formatter.config;
        let fields = [
            ("timestamp", cfg.timestamp),
            ("level", cfg.level),
            ("tag", cfg.tag),
            ("pid", cfg.pid),
            ("tid", cfg.tid),
            ("message", cfg.message),
        ];
        let mut line = String::from("Fields: ");
        for (name, on) in fields {
            let (sign, color) = if on { ("+", "32") } else { ("-", "31") };
            line.push_str(&format!("\x1b[{color}m{sign}{name}\x1b[0m "));
        }
        ctx.output.push(line);
        return;
    }
    for token in args.split_whitespace() {
        if token.len() < 2 {
            ctx.output.push(format!("\x1b[31mInvalid: {token}\x1b[0m"));
            continue;
        }
        let enabled = token.starts_with('+');
        let name = &token[1..];
        if ctx.formatter.config.toggle_field(name, enabled) {
            let state = if enabled { "on" } else { "off" };
            ctx.output.push(format!("\x1b[32m{name}: {state}\x1b[0m"));
        } else {
            ctx.output.push(format!("\x1b[31mUnknown field: {name}\x1b[0m"));
        }
    }
}

fn cmd_stop(ctx: &mut CommandContext) {
    *ctx.streaming = false;
    *ctx.paused = false;
    ctx.output.push("\x1b[33mStopped — log stream ended. Use /app or /pid to restart.\x1b[0m".to_string());
}

fn cmd_pause(ctx: &mut CommandContext) {
    // Toggle pause
    if *ctx.paused {
        *ctx.paused = false;
        ctx.output.push("\x1b[32mResumed\x1b[0m".to_string());
    } else {
        *ctx.paused = true;
        ctx.output.push("\x1b[33mPaused — /pause again to resume\x1b[0m".to_string());
    }
}

fn cmd_resume(ctx: &mut CommandContext) {
    *ctx.paused = false;
    ctx.output.push("\x1b[32mResumed\x1b[0m".to_string());
}

fn cmd_save(ctx: &mut CommandContext, args: &str) {
    let arg = args.trim();
    if arg.is_empty() {
        // Empty arg disables saving
        *ctx.save_path = None;
        ctx.output.push("\x1b[33mSave stopped\x1b[0m".to_string());
        return;
    }

    // Expand ~ to $HOME
    let expanded: String = if let Some(rest) = arg.strip_prefix("~/") {
        match std::env::var("HOME") {
            Ok(home) => format!("{home}/{rest}"),
            Err(_) => arg.to_string(),
        }
    } else if arg == "~" {
        std::env::var("HOME").unwrap_or_else(|_| arg.to_string())
    } else {
        arg.to_string()
    };

    // Validate: parent dir must exist and file must be creatable
    let path = std::path::Path::new(&expanded);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            ctx.output.push(format!(
                "\x1b[31mSave failed: directory does not exist: {}\x1b[0m",
                parent.display()
            ));
            return;
        }
    }

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&expanded)
    {
        Ok(_) => {
            *ctx.save_path = Some(expanded.clone());
            ctx.output
                .push(format!("\x1b[32mSaving matching logs to: {expanded}\x1b[0m"));
        }
        Err(e) => {
            ctx.output
                .push(format!("\x1b[31mSave failed: {expanded}: {e}\x1b[0m"));
        }
    }
}

fn cmd_preset(ctx: &mut CommandContext, args: &str) {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    let sub = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let name = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match sub.as_str() {
        "save" => {
            if name.is_empty() {
                ctx.output.push("\x1b[31mUsage: /preset save <name>\x1b[0m".to_string());
                return;
            }
            match config::save_preset(name, ctx.filters, &ctx.formatter.config) {
                Ok(path) => ctx.output.push(format!("\x1b[32mPreset saved: {name} -> {}\x1b[0m", path.display())),
                Err(e) => ctx.output.push(format!("\x1b[31mError: {e}\x1b[0m")),
            }
        }
        "load" => {
            if name.is_empty() {
                ctx.output.push("\x1b[31mUsage: /preset load <name>\x1b[0m".to_string());
                return;
            }
            match config::load_preset(name, ctx.filters, &mut ctx.formatter.config) {
                Ok(()) => {
                    ctx.output.push(format!("\x1b[32mPreset loaded: {name}\x1b[0m"));
                    // Track in filter history for current app
                    if !ctx.filters.package.is_empty() {
                        config::save_filter_to_history(&ctx.filters.package, name);
                    }
                }
                Err(e) => ctx.output.push(format!("\x1b[31m{e}\x1b[0m")),
            }
        }
        "list" => {
            let presets = config::list_presets();
            if presets.is_empty() {
                ctx.output.push("\x1b[2mNo saved presets\x1b[0m".to_string());
            } else {
                ctx.output.push(format!("\x1b[36mSaved presets:\x1b[0m {}", presets.join(", ")));
            }
        }
        "delete" => {
            if name.is_empty() {
                ctx.output.push("\x1b[31mUsage: /preset delete <name>\x1b[0m".to_string());
                return;
            }
            if config::delete_preset(name) {
                ctx.output.push(format!("\x1b[32mDeleted: {name}\x1b[0m"));
            } else {
                ctx.output.push(format!("\x1b[31mNot found: {name}\x1b[0m"));
            }
        }
        _ => ctx.output.push("\x1b[31mUsage: /preset save|load|list|delete <name>\x1b[0m".to_string()),
    }
}

async fn cmd_traffic(ctx: &mut CommandContext<'_>, args: &str) {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    let sub = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let rest = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match sub.as_str() {
        "open" => {
            match ctx.traffic.start().await {
                Ok(msg) => {
                    ctx.output.push(format!("\x1b[32m{msg}\x1b[0m"));
                    ctx.output.push(format!("\x1b[2mConfigure device proxy to port {}\x1b[0m", ctx.traffic.listen_port));
                }
                Err(msg) => ctx.output.push(format!("\x1b[31m{msg}\x1b[0m")),
            }
        }
        "close" => {
            match ctx.traffic.stop().await {
                Ok(msg) => ctx.output.push(format!("\x1b[33m{msg}\x1b[0m")),
                Err(msg) => ctx.output.push(format!("\x1b[31m{msg}\x1b[0m")),
            }
        }
        "list" => {
            let state = ctx.traffic.state.lock().unwrap();
            let entries = state.get_filtered(50);
            if entries.is_empty() {
                ctx.output.push("\x1b[2mNo traffic captured\x1b[0m".to_string());
                return;
            }
            ctx.output.push(format!(
                "\x1b[1m{:<5} {:<12} {:<7} {:<6} {:<24} {}\x1b[0m",
                "#", "Time", "Method", "Status", "Host", "Path"
            ));
            for e in entries {
                let status_color = match e.status {
                    Some(s) if s < 400 => "32",
                    Some(_) => "31",
                    None => "2",
                };
                let status_str = e.status.map(|s| s.to_string()).unwrap_or_else(|| "...".to_string());
                ctx.output.push(format!(
                    "  {:<5} {:<12} {:<7} \x1b[{status_color}m{:<6}\x1b[0m {:<24} {}",
                    e.id, e.timestamp, e.method, status_str, e.host, e.path,
                ));
            }
        }
        "inspect" => {
            let id: usize = match rest.parse() {
                Ok(id) => id,
                Err(_) => {
                    ctx.output.push("\x1b[31mUsage: /traffic inspect <id>\x1b[0m".to_string());
                    return;
                }
            };
            let state = ctx.traffic.state.lock().unwrap();
            match state.get_entry(id) {
                Some(e) => {
                    ctx.output.push(format!("\x1b[1m--- Traffic #{} ---\x1b[0m", e.id));
                    ctx.output.push(format!("\x1b[1m{}\x1b[0m {}", e.method, e.url));
                    ctx.output.push(format!("Status: {}", e.status.map(|s| s.to_string()).unwrap_or("pending".to_string())));
                    ctx.output.push(String::new());
                    ctx.output.push("\x1b[1mRequest Headers:\x1b[0m".to_string());
                    for (k, v) in &e.request_headers {
                        ctx.output.push(format!("  {k}: {v}"));
                    }
                    if !e.request_body.is_empty() {
                        ctx.output.push(String::new());
                        ctx.output.push("\x1b[1mRequest Body:\x1b[0m".to_string());
                        ctx.output.push(String::from_utf8_lossy(&e.request_body[..e.request_body.len().min(2000)]).to_string());
                    }
                    ctx.output.push(String::new());
                    ctx.output.push("\x1b[1mResponse Headers:\x1b[0m".to_string());
                    for (k, v) in &e.response_headers {
                        ctx.output.push(format!("  {k}: {v}"));
                    }
                    if !e.response_body.is_empty() {
                        ctx.output.push(String::new());
                        ctx.output.push("\x1b[1mResponse Body:\x1b[0m".to_string());
                        ctx.output.push(String::from_utf8_lossy(&e.response_body[..e.response_body.len().min(2000)]).to_string());
                    }
                }
                None => ctx.output.push(format!("\x1b[31mEntry #{rest} not found\x1b[0m")),
            }
        }
        "filter" => {
            let mut state = ctx.traffic.state.lock().unwrap();
            if rest.is_empty() {
                state.filter.reset();
                ctx.output.push("\x1b[32mTraffic filter cleared\x1b[0m".to_string());
                return;
            }
            for pair in rest.split_whitespace() {
                if let Some((k, v)) = pair.split_once('=') {
                    match k.to_lowercase().as_str() {
                        "host" => state.filter.host = v.to_string(),
                        "path" => state.filter.path = v.to_string(),
                        "method" => state.filter.method = v.to_string(),
                        "status" => state.filter.status = v.parse().ok(),
                        "body" => state.filter.body_search = v.to_string(),
                        _ => {}
                    }
                }
            }
            ctx.output.push("\x1b[32mTraffic filter updated\x1b[0m".to_string());
        }
        "clear" => {
            ctx.traffic.state.lock().unwrap().clear();
            ctx.output.push("\x1b[32mTraffic cleared\x1b[0m".to_string());
        }
        _ => ctx.output.push("\x1b[31mUsage: /traffic open|close|list|inspect|filter|clear\x1b[0m".to_string()),
    }
}

fn cmd_mock(ctx: &mut CommandContext, args: &str) {
    let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
    let sub = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let rest = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match sub.as_str() {
        "load" => {
            if rest.is_empty() {
                ctx.output.push("\x1b[31mUsage: /mock load <rules.yaml>\x1b[0m".to_string());
                return;
            }
            match ctx.mock_engine.load(rest) {
                Ok(msg) => ctx.output.push(format!("\x1b[32m{msg}\x1b[0m")),
                Err(msg) => ctx.output.push(format!("\x1b[31m{msg}\x1b[0m")),
            }
        }
        "list" => {
            if ctx.mock_engine.rules.is_empty() {
                ctx.output.push("\x1b[2mNo rules loaded\x1b[0m".to_string());
                return;
            }
            ctx.output.push(format!(
                "\x1b[1m{:<20} {:<8} {:<20} {:<16} {}\x1b[0m",
                "ID", "Enabled", "Match", "Response", "Hits"
            ));
            for rule in &ctx.mock_engine.rules {
                let enabled = if rule.enabled { "\x1b[32mON\x1b[0m" } else { "\x1b[31mOFF\x1b[0m" };
                let method = if rule.match_rule.method.is_empty() { "*" } else { &rule.match_rule.method };
                let path = if rule.match_rule.path.is_empty() { "*" } else { &rule.match_rule.path };
                let match_desc = format!("{method} {path}");
                let resp_desc = format!("{} -> {}", rule.response.resp_type, rule.response.status);
                ctx.output.push(format!(
                    "  {:<20} {:<17} {:<20} {:<16} {}",
                    rule.id, enabled, match_desc, resp_desc, rule.hit_count
                ));
            }
        }
        "enable" => {
            if rest.is_empty() {
                ctx.output.push("\x1b[31mUsage: /mock enable <rule_id>\x1b[0m".to_string());
                return;
            }
            if ctx.mock_engine.enable_rule(rest) {
                ctx.output.push(format!("\x1b[32mEnabled: {rest}\x1b[0m"));
            } else {
                ctx.output.push(format!("\x1b[31mRule not found: {rest}\x1b[0m"));
            }
        }
        "disable" => {
            if rest.is_empty() {
                ctx.output.push("\x1b[31mUsage: /mock disable <rule_id>\x1b[0m".to_string());
                return;
            }
            if ctx.mock_engine.disable_rule(rest) {
                ctx.output.push(format!("\x1b[33mDisabled: {rest}\x1b[0m"));
            } else {
                ctx.output.push(format!("\x1b[31mRule not found: {rest}\x1b[0m"));
            }
        }
        "reload" => {
            match ctx.mock_engine.reload() {
                Ok(msg) => ctx.output.push(format!("\x1b[32m{msg}\x1b[0m")),
                Err(msg) => ctx.output.push(format!("\x1b[31m{msg}\x1b[0m")),
            }
        }
        _ => ctx.output.push("\x1b[31mUsage: /mock load|list|enable|disable|reload\x1b[0m".to_string()),
    }
}
