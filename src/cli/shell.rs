//! Interactive CLI shell — REPL with async log streaming.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::adb::AdbClient;
use crate::logs::filters::{self, FilterState};
use crate::logs::formatter::LogFormatter;
use crate::logs::parser::parse_logcat_line;
use crate::mock::MockEngine;
use crate::traffic::TrafficProxy;

use super::commands::{dispatch, CommandContext};
use super::completer::LoguxHelper;

const BANNER: &str = r#"
 ╦  ╔═╗╔═╗╦ ╦═╗ ╦
 ║  ║ ║║ ╦║ ║╔╩╦╝
 ╩═╝╚═╝╚═╝╚═╝╩ ╚═  v2.0
"#;

pub async fn run() {
    println!("\x1b[1;36m{BANNER}\x1b[0m");
    println!("\x1b[2mType /help for commands, /exit to quit\x1b[0m\n");

    let mut adb = AdbClient::new();
    let mut filters = FilterState::default();
    let mut formatter = LogFormatter::default();
    let mut traffic = TrafficProxy::new(8888);
    let mut mock_engine = MockEngine::new();
    let mut streaming = false;
    let mut paused = false;
    let mut save_path: Option<String> = None;
    let mut exit_requested = false;

    // Check ADB
    let (ok, version) = adb.check_adb();
    if ok {
        println!("\x1b[32mADB: {version}\x1b[0m");
    } else {
        println!("\x1b[31mADB: {version}\x1b[0m");
    }

    // List devices
    {
        let devices = adb.list_devices();
        let online_count = devices.iter().filter(|d| d.is_online()).count();
        let total = devices.len();
        if total > 0 {
            println!("\x1b[2mDevices: {online_count} online / {total} total\x1b[0m");
            if online_count == 1 {
                let dev = devices.iter().find(|d| d.is_online()).unwrap().clone();
                let name = dev.display_name();
                adb.selected_device = Some(dev);
                println!("\x1b[32mAuto-selected: {name}\x1b[0m");
            }
        } else {
            println!("\x1b[33mNo devices connected\x1b[0m");
        }
    }
    println!();

    // History file
    let history_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".logux");
    let _ = std::fs::create_dir_all(&history_dir);
    let history_file = history_dir.join("history");

    let mut rl = Editor::new().unwrap();
    rl.set_max_history_size(1000).unwrap();
    rl.set_helper(Some(LoguxHelper::new()));
    let _ = rl.load_history(&history_file);

    // Shared flag for log streaming
    let running = Arc::new(AtomicBool::new(false));
    let stream_paused = Arc::new(AtomicBool::new(false));

    loop {
        // Build prompt
        let prompt = build_prompt(&adb, &filters, streaming, paused, traffic.is_running());

        // If we should start streaming and not yet running
        if streaming && !running.load(Ordering::Relaxed) {
            let running_clone = running.clone();
            let paused_clone = stream_paused.clone();
            let formatter_clone_config = formatter.config.clone();
            let highlight = formatter.highlight_text.clone();
            let filter_desc = (
                filters.pids.clone(),
                filters.tags.clone(),
                filters.min_level,
                filters.text.clone(),
                filters.regex.as_ref().map(|r| r.as_str().to_string()),
                filters.threads.clone(),
            );
            let save = save_path.clone();

            if let Ok(child) = adb.start_logcat(false) {
                running_clone.store(true, Ordering::Relaxed);
                tokio::spawn(async move {
                    stream_logs(child, formatter_clone_config, highlight, filter_desc, paused_clone, running_clone, save).await;
                });
            }
        }

        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);

                if line.starts_with('/') {
                    let mut ctx = CommandContext {
                        adb: &mut adb,
                        filters: &mut filters,
                        formatter: &mut formatter,
                        traffic: &mut traffic,
                        mock_engine: &mut mock_engine,
                        streaming: &mut streaming,
                        paused: &mut paused,
                        save_path: &mut save_path,
                        exit_requested: &mut exit_requested,
                    };
                    dispatch(&mut ctx, &line).await;

                    stream_paused.store(paused, Ordering::Relaxed);

                    if exit_requested {
                        break;
                    }

                    // If streaming state changed, stop current stream
                    if !streaming {
                        running.store(false, Ordering::Relaxed);
                    }
                } else {
                    // Quick filter shortcut
                    filters.set_text(&line);
                    formatter.highlight_text = line.clone();
                    println!("\x1b[32mQuick filter: '{line}'\x1b[0m");
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
        }
    }

    // Cleanup
    running.store(false, Ordering::Relaxed);
    let _ = traffic.stop().await;
    let _ = rl.save_history(&history_file);
    println!("\n\x1b[2mBye!\x1b[0m");
}

fn build_prompt(
    adb: &AdbClient,
    filters: &FilterState,
    streaming: bool,
    paused: bool,
    proxy: bool,
) -> String {
    let mut prompt = String::from("\x1b[1;36mlogux\x1b[0m");

    if let Some(ref dev) = adb.selected_device {
        let name = if !dev.model.is_empty() { &dev.model } else { &dev.serial };
        prompt.push_str(&format!("@{name}"));
    }

    if !filters.package.is_empty() {
        prompt.push_str(&format!("\x1b[33m [{}]\x1b[0m", filters.package));
    }

    if paused {
        prompt.push_str("\x1b[31m (paused)\x1b[0m");
    } else if streaming {
        prompt.push_str("\x1b[32m (streaming)\x1b[0m");
    }

    if proxy {
        prompt.push_str("\x1b[35m (proxy)\x1b[0m");
    }

    prompt.push_str("\x1b[1;36m > \x1b[0m");
    prompt
}

async fn stream_logs(
    mut child: tokio::process::Child,
    config: crate::logs::formatter::FormatConfig,
    highlight: String,
    filter_state: (
        std::collections::HashSet<u32>,
        std::collections::HashSet<String>,
        crate::logs::parser::LogLevel,
        String,
        Option<String>,
        std::collections::HashSet<u32>,
    ),
    paused: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    save_path: Option<String>,
) {
    let stdout = match child.stdout.take() {
        Some(out) => out,
        None => return,
    };

    let mut reader = BufReader::new(stdout).lines();
    let formatter = LogFormatter {
        config,
        highlight_text: highlight,
    };

    // Build a filter state from the cloned data
    let mut fs = FilterState::default();
    fs.pids = filter_state.0;
    fs.tags = filter_state.1;
    fs.min_level = filter_state.2;
    fs.text = filter_state.3;
    if let Some(pattern) = filter_state.4 {
        let _ = fs.set_regex(&pattern);
    }
    fs.threads = filter_state.5;

    let mut save_file = save_path.and_then(|p| {
        OpenOptions::new().create(true).append(true).open(p).ok()
    });

    while running.load(Ordering::Relaxed) {
        match reader.next_line().await {
            Ok(Some(line)) => {
                if let Some(entry) = parse_logcat_line(&line) {
                    if !filters::matches(&entry, &fs) {
                        continue;
                    }
                    if paused.load(Ordering::Relaxed) {
                        continue;
                    }
                    let formatted = formatter.format_entry(&entry);
                    println!("{formatted}");

                    if let Some(ref mut f) = save_file {
                        let _ = writeln!(f, "{}", entry.raw);
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let _ = child.kill().await;
}
