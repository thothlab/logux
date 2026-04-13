//! Command completion — standalone completion logic for the TUI.

pub const COMMANDS: &[(&str, &[&str])] = &[
    ("/help", &[]),
    ("/exit", &[]),
    ("/clear", &[]),
    ("/devices", &[]),
    ("/connect", &[]),
    ("/disconnect", &[]),
    ("/app", &[]),
    ("/pid", &[]),
    ("/tag", &["reset"]),
    ("/level", &["verbose", "debug", "info", "warn", "error", "fatal"]),
    ("/grep", &["reset"]),
    ("/regex", &["reset"]),
    ("/filter", &["reset", "show", "edit", "set", "tag", "level", "grep", "regex", "exclude", "app"]),
    ("/exclude", &["tag", "msg", "show", "reset", "remove"]),
    ("/format", &["compact", "threadtime", "verbose", "minimal", "json"]),
    ("/fields", &["+timestamp", "-timestamp", "+level", "-level", "+tag", "-tag", "+pid", "-pid", "+tid", "-tid"]),
    ("/stop", &[]),
    ("/pause", &[]),
    ("/resume", &[]),
    ("/save", &[]),
    ("/preset", &["save", "load", "list", "delete"]),
    ("/traffic", &["open", "close", "list", "inspect", "filter", "clear"]),
    ("/mock", &["load", "list", "enable", "disable", "reload"]),
];

/// Complete the input, returning a list of full suggestion strings.
pub fn complete(
    input: &str,
    app_history: &[String],
    foreground_package: Option<&str>,
    current_package: &str,
) -> Vec<String> {
    if !input.starts_with('/') {
        return vec![];
    }

    let parts: Vec<&str> = input.splitn(2, char::is_whitespace).collect();

    // Still typing the command name (no space yet)
    if parts.len() == 1 && !input.ends_with(' ') {
        let prefix = parts[0];
        return COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix))
            .map(|(cmd, _)| cmd.to_string())
            .collect();
    }

    let cmd = parts[0];
    let arg_text = parts.get(1).unwrap_or(&"").trim_start();

    // Special: /app — show history + foreground package
    if cmd == "/app" {
        let mut suggestions = Vec::new();
        // Currently running foreground app
        if let Some(fg) = foreground_package {
            if !fg.is_empty() && fg.contains(arg_text) {
                let full = format!("/app {fg}");
                suggestions.push(full);
            }
        }
        // Current package if set
        if !current_package.is_empty() && current_package.contains(arg_text) {
            let full = format!("/app {current_package}");
            if !suggestions.contains(&full) {
                suggestions.push(full);
            }
        }
        // History items
        for pkg in app_history {
            let full = format!("/app {pkg}");
            if pkg.contains(arg_text) && !suggestions.contains(&full) {
                suggestions.push(full);
            }
        }
        return suggestions;
    }

    // Special: /filter — show presets associated with current app + reset/show
    if cmd == "/filter" {
        let mut suggestions = Vec::new();
        // Standard subcommands
        for sub in &["reset", "show"] {
            if sub.starts_with(arg_text) {
                suggestions.push(format!("/filter {sub}"));
            }
        }
        // Filter history presets for current app
        if !current_package.is_empty() {
            let history = crate::config::load_filter_history(current_package);
            for preset in history {
                if preset.contains(arg_text) {
                    let full = format!("/filter {preset}");
                    if !suggestions.contains(&full) {
                        suggestions.push(full);
                    }
                }
            }
        }
        // All presets as fallback
        let all_presets = crate::config::list_presets();
        for p in all_presets {
            if p.contains(arg_text) {
                let full = format!("/filter {p}");
                if !suggestions.contains(&full) {
                    suggestions.push(full);
                }
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
            .map(|p| format!("/preset load {p}"))
            .collect();
    }

    // Standard subcommand completion
    if let Some((_, subs)) = COMMANDS.iter().find(|(c, _)| *c == cmd) {
        return subs
            .iter()
            .filter(|s| s.starts_with(arg_text))
            .map(|s| format!("{cmd} {s}"))
            .collect();
    }

    vec![]
}
