//! Command completion — standalone completion logic for the TUI.

/// (command, description, subcommands)
pub const COMMANDS: &[(&str, &str, &[&str])] = &[
    ("/help", "Show help for all commands", &[]),
    ("/exit", "Exit logux", &[]),
    ("/clear", "Clear the log view", &[]),
    ("/devices", "List connected ADB devices", &[]),
    ("/connect", "Connect to device over TCP (ip:port)", &[]),
    ("/disconnect", "Disconnect current device", &[]),
    ("/app", "Filter by app package (with PID tracking)", &[]),
    ("/pid", "Filter by process ID", &[]),
    ("/tag", "Add tag filter (reset to clear)", &["reset"]),
    ("/level", "Set minimum log level", &["verbose", "debug", "info", "warn", "error", "fatal"]),
    ("/grep", "Search by text in tag + message", &["reset"]),
    ("/msg", "Search by text in message only", &["reset"]),
    ("/regex", "Filter by regex pattern", &["reset"]),
    ("/filter", "Edit, show, or load filter presets", &["reset", "show", "edit", "set", "tag", "level", "grep", "msg", "regex", "exclude", "app"]),
    ("/exclude", "Exclude tags or messages from output", &["tag", "msg", "show", "reset", "remove"]),
    ("/format", "Switch output format preset", &["compact", "threadtime", "verbose", "minimal", "json"]),
    ("/fields", "Toggle visible columns", &["+timestamp", "-timestamp", "+level", "-level", "+tag", "-tag", "+pid", "-pid", "+tid", "-tid"]),
    ("/stop", "Stop the log stream completely", &[]),
    ("/pause", "Pause or resume display (toggle)", &[]),
    ("/resume", "Resume display after pause", &[]),
    ("/save", "Save matching logs to file", &[]),
    ("/preset", "Save and load configuration presets", &["save", "load", "list", "delete"]),
    ("/traffic", "HTTP(S) proxy inspection", &["open", "close", "list", "inspect", "filter", "clear"]),
    ("/mock", "YAML-based mock response rules", &["load", "list", "enable", "disable", "reload"]),
    ("/mouse", "Toggle mouse capture (enables wheel scroll; blocks selection)", &["on", "off", "toggle"]),
    ("/copy", "Copy last N log messages (no column padding) to clipboard", &[]),
    ("/width", "Set column widths: /width tag=30 ts=25 …", &["show", "reset", "timestamp=", "level=", "tag=", "pid=", "tid="]),
    ("/forget", "Clear all auto-saved filter presets and per-app filter memory", &[]),
];

/// Description for a /filter subcommand.
fn filter_sub_desc(sub: &str) -> &'static str {
    match sub {
        "reset" => "Clear all filters",
        "show" => "Show active filters",
        "edit" => "Edit filters inline",
        "set" => "Apply filters from a key=value string",
        "tag" => "Add/remove tag filter",
        "level" => "Set minimum level",
        "grep" => "Text search (tag + message)",
        "msg" => "Text search (message only)",
        "regex" => "Regex filter",
        "exclude" => "Manage exclusion filters",
        "app" => "Set app package filter",
        _ => "",
    }
}

/// A single completion entry.
/// `text` is inserted on Tab. `display` is shown in the suggestion list
/// (may differ from `text`, e.g. "/exit (quit)" display vs "/exit" insert).
pub struct Suggestion {
    pub text: String,
    pub display: String,
    pub desc: String,
}

impl Suggestion {
    fn new<S: Into<String>, D: Into<String>>(text: S, desc: D) -> Self {
        let text = text.into();
        Self { display: text.clone(), text, desc: desc.into() }
    }

    fn with_display<S: Into<String>, DS: Into<String>, D: Into<String>>(
        text: S,
        display: DS,
        desc: D,
    ) -> Self {
        Self {
            text: text.into(),
            display: display.into(),
            desc: desc.into(),
        }
    }
}

/// Command aliases: (alias, canonical).
/// Typing any alias as a prefix also surfaces the canonical command.
const ALIASES: &[(&str, &str)] = &[("/quit", "/exit")];

/// Display string for a canonical command, including alias hints.
/// E.g. `/exit` → `/exit (quit)`.
fn command_display(cmd: &str) -> String {
    let aliases: Vec<&str> = ALIASES
        .iter()
        .filter(|(_, canon)| *canon == cmd)
        .map(|(alias, _)| alias.trim_start_matches('/'))
        .collect();
    if aliases.is_empty() {
        cmd.to_string()
    } else {
        format!("{cmd} ({})", aliases.join(", "))
    }
}

fn push_unique(list: &mut Vec<Suggestion>, s: Suggestion) {
    if !list.iter().any(|x| x.text == s.text) {
        list.push(s);
    }
}

/// Complete the input, returning a list of (text, description) suggestions.
pub fn complete(
    input: &str,
    app_history: &[String],
    foreground_package: Option<&str>,
    current_package: &str,
) -> Vec<Suggestion> {
    if !input.starts_with('/') {
        return vec![];
    }

    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();

    // Still typing the command name (no space yet)
    if parts.len() == 1 && !input.ends_with(' ') {
        let prefix = parts[0];
        let mut matched: Vec<&str> = COMMANDS
            .iter()
            .filter(|(cmd, _, _)| cmd.starts_with(prefix))
            .map(|(cmd, _, _)| *cmd)
            .collect();
        // Also match aliases → surface their canonical command
        for (alias, canon) in ALIASES {
            if alias.starts_with(prefix) && !matched.iter().any(|c| c == canon) {
                matched.push(canon);
            }
        }
        return matched
            .iter()
            .filter_map(|cmd| {
                COMMANDS
                    .iter()
                    .find(|(c, _, _)| c == cmd)
                    .map(|(c, desc, _)| {
                        Suggestion::with_display(*c, command_display(c), *desc)
                    })
            })
            .collect();
    }

    let cmd = parts[0];
    let arg_text = parts.get(1).unwrap_or(&"").trim_start();

    // Special: /app — show history + foreground package
    if cmd == "/app" {
        let mut suggestions: Vec<Suggestion> = Vec::new();
        if let Some(fg) = foreground_package {
            if !fg.is_empty() && fg.contains(arg_text) {
                push_unique(&mut suggestions, Suggestion::new(format!("/app {fg}"), "foreground app"));
            }
        }
        if !current_package.is_empty() && current_package.contains(arg_text) {
            push_unique(&mut suggestions, Suggestion::new(format!("/app {current_package}"), "current package"));
        }
        for pkg in app_history {
            if pkg.contains(arg_text) {
                push_unique(&mut suggestions, Suggestion::new(format!("/app {pkg}"), "from history"));
            }
        }
        return suggestions;
    }

    // Special: /filter — show subcommands + presets associated with current app
    if cmd == "/filter" {
        let mut suggestions: Vec<Suggestion> = Vec::new();
        for sub in &["reset", "show", "edit", "set", "tag", "level", "grep", "msg", "regex", "exclude", "app"] {
            if sub.starts_with(arg_text) {
                suggestions.push(Suggestion::new(format!("/filter {sub}"), filter_sub_desc(sub)));
            }
        }
        if !current_package.is_empty() {
            let history = crate::config::load_filter_history(current_package);
            for preset in history {
                if preset.contains(arg_text) {
                    push_unique(&mut suggestions, Suggestion::new(format!("/filter {preset}"), "preset (app history)"));
                }
            }
        }
        let all_presets = crate::config::list_presets();
        for p in all_presets {
            if p.contains(arg_text) {
                push_unique(&mut suggestions, Suggestion::new(format!("/filter {p}"), "saved preset"));
            }
        }
        return suggestions;
    }

    // Special: /preset load — show preset names
    if cmd == "/preset" && (arg_text.starts_with("load ") || arg_text == "load") {
        let presets = crate::config::list_presets();
        let filter_text = arg_text.strip_prefix("load").unwrap_or("").trim();
        return presets
            .iter()
            .filter(|p| p.contains(filter_text))
            .map(|p| Suggestion::new(format!("/preset load {p}"), "saved preset"))
            .collect();
    }

    // Standard subcommand completion
    if let Some((_, _, subs)) = COMMANDS.iter().find(|(c, _, _)| *c == cmd) {
        return subs
            .iter()
            .filter(|s| s.starts_with(arg_text))
            .map(|s| {
                let desc = if cmd == "/filter" { filter_sub_desc(s) } else { "" };
                Suggestion::new(format!("{cmd} {s}"), desc)
            })
            .collect();
    }

    vec![]
}
