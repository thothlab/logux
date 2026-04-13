//! Command auto-completer for rustyline REPL.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

const COMMANDS: &[(&str, &[&str])] = &[
    ("/help", &[]),
    ("/exit", &[]),
    ("/clear", &[]),
    ("/devices", &[]),
    ("/connect", &[]),
    ("/disconnect", &[]),
    ("/app", &[]),
    ("/pid", &[]),
    ("/tag", &[]),
    ("/level", &["verbose", "debug", "info", "warn", "error", "fatal"]),
    ("/grep", &[]),
    ("/regex", &[]),
    ("/filter", &["reset", "show"]),
    ("/format", &["compact", "threadtime", "verbose", "minimal", "json"]),
    ("/fields", &["+timestamp", "-timestamp", "+level", "-level", "+tag", "-tag", "+pid", "-pid", "+tid", "-tid"]),
    ("/pause", &[]),
    ("/resume", &[]),
    ("/save", &[]),
    ("/preset", &["save", "load", "list", "delete"]),
    ("/traffic", &["open", "close", "list", "inspect", "filter", "clear"]),
    ("/mock", &["load", "list", "enable", "disable", "reload"]),
];

pub struct LoguxHelper {
    hinter: HistoryHinter,
}

impl LoguxHelper {
    pub fn new() -> Self {
        Self {
            hinter: HistoryHinter::new(),
        }
    }
}

impl Completer for LoguxHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let text = &line[..pos];
        if !text.starts_with('/') {
            return Ok((0, vec![]));
        }

        let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();

        if parts.len() == 1 && !text.ends_with(' ') {
            // Complete command name
            let prefix = parts[0];
            let matches: Vec<Pair> = COMMANDS
                .iter()
                .filter(|(cmd, _)| cmd.starts_with(prefix))
                .map(|(cmd, _)| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            return Ok((0, matches));
        }

        // Complete subcommand
        let cmd = parts[0];
        let arg_text = parts.get(1).unwrap_or(&"").trim_start();
        if let Some((_, subs)) = COMMANDS.iter().find(|(c, _)| *c == cmd) {
            let matches: Vec<Pair> = subs
                .iter()
                .filter(|s| s.starts_with(arg_text))
                .map(|s| Pair {
                    display: s.to_string(),
                    replacement: s.to_string(),
                })
                .collect();
            let start = pos - arg_text.len();
            return Ok((start, matches));
        }

        Ok((0, vec![]))
    }
}

impl Hinter for LoguxHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for LoguxHelper {}
impl Validator for LoguxHelper {}
impl Helper for LoguxHelper {}
