//! Interactive REPL for amem — slash command interface.
//!
//! Launch with `amem` (no subcommand) to enter interactive mode.
//! Type `/help` for available commands, Tab for completion.

use crate::cli::repl_commands;
use crate::cli::repl_complete;
use rustyline::config::CompletionType;
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor};

/// History file location.
fn history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".amem_history")
}

/// Print the welcome banner.
fn print_banner() {
    eprintln!();
    eprintln!(
        "  \x1b[32m\u{25c9}\x1b[0m \x1b[1mamem v{}\x1b[0m \x1b[90m\u{2014} Binary Graph Memory for AI Agents\x1b[0m",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!();
    eprintln!(
        "    Press \x1b[36m/\x1b[0m to browse commands, \x1b[90mTab\x1b[0m to complete, \x1b[90m/exit\x1b[0m to quit."
    );
    eprintln!();
}

/// Run the interactive REPL.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    print_banner();

    let config = Config::builder()
        .history_ignore_space(true)
        .auto_add_history(true)
        .completion_type(CompletionType::List)
        .completion_prompt_limit(20)
        .build();

    let helper = repl_complete::AmemHelper::new();
    let mut rl: Editor<repl_complete::AmemHelper, rustyline::history::DefaultHistory> =
        Editor::with_config(config)?;
    rl.set_helper(Some(helper));
    repl_complete::bind_keys(&mut rl);

    let hist_path = history_path();
    if hist_path.exists() {
        let _ = rl.load_history(&hist_path);
    }

    let mut state = repl_commands::ReplState::new();
    let prompt = " \x1b[36mamem>\x1b[0m ";

    loop {
        match rl.readline(prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match repl_commands::execute(line, &mut state) {
                    Ok(true) => {
                        eprintln!("  \x1b[90m\u{2728}\x1b[0m Goodbye!");
                        break;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!("  Error: {e}");
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                eprintln!("  \x1b[90m(Ctrl+C)\x1b[0m Type \x1b[1m/exit\x1b[0m to quit.");
            }
            Err(ReadlineError::Eof) => {
                eprintln!("  \x1b[90m\u{2728}\x1b[0m Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("  Error: {err}");
                break;
            }
        }
    }

    let _ = std::fs::create_dir_all(hist_path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = rl.save_history(&hist_path);

    Ok(())
}
