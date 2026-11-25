pub mod args;
pub mod commands;

use crate::backend::Backend;
use crate::config::Config;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

/// Everything the commands need from the terminal. Tests script it.
pub trait Io {
    fn out(&mut self, s: &str);
    fn err(&mut self, s: &str);
    /// Show `prompt` and read one line; None on EOF.
    fn ask(&mut self, prompt: &str) -> Option<String>;
    /// Can we prompt the user (stdin is a terminal)?
    fn interactive(&self) -> bool;
    fn stdout_is_tty(&self) -> bool;
    /// Colour on stdout is allowed (a terminal and `NO_COLOR` unset).
    fn color_allowed(&self) -> bool;
    fn stderr_is_tty(&self) -> bool;
    fn stderr_color_allowed(&self) -> bool;
    /// Safe to print symbols such as ✓ and ┃ (UTF-8 locale); otherwise ASCII is used.
    fn unicode(&self) -> bool;
}

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

fn utf8_locale() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .is_some_and(|v| {
            let v = v.to_ascii_lowercase();
            v.contains("utf-8") || v.contains("utf8")
        })
}

pub struct StdIo;

impl Io for StdIo {
    fn out(&mut self, s: &str) {
        let mut o = std::io::stdout().lock();
        // A closed pipe (`ifhf search x | head`) is not an error worth a panic.
        if o.write_all(s.as_bytes()).is_err() {
            std::process::exit(0);
        }
    }
    fn err(&mut self, s: &str) {
        let _ = std::io::stderr().write_all(s.as_bytes());
    }
    fn ask(&mut self, prompt: &str) -> Option<String> {
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(prompt.as_bytes());
        let _ = o.flush();
        drop(o);
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }
    fn interactive(&self) -> bool {
        std::io::stdin().is_terminal()
    }
    fn stdout_is_tty(&self) -> bool {
        std::io::stdout().is_terminal()
    }
    fn color_allowed(&self) -> bool {
        self.stdout_is_tty() && !no_color_env()
    }
    fn stderr_is_tty(&self) -> bool {
        std::io::stderr().is_terminal()
    }
    fn stderr_color_allowed(&self) -> bool {
        self.stderr_is_tty() && !no_color_env()
    }
    fn unicode(&self) -> bool {
        utf8_locale()
    }
}

pub struct Env<'a> {
    pub backend: &'a dyn Backend,
    pub config: &'a Config,
    pub cache_dir: Option<PathBuf>,
    pub io: &'a mut dyn Io,
}

/// Parse and execute; returns the process exit code. All user-facing errors are printed here.
pub fn run(args: &[String], env: &mut Env) -> i32 {
    let result = args::parse(args).and_then(|inv| commands::execute(env, inv));
    match result {
        Ok(()) => 0,
        Err(e) => {
            let tail = if matches!(e, crate::error::Error::Usage(_)) {
                "\n\nTry: ifhf --help"
            } else {
                ""
            };
            let fancy = env.io.stderr_is_tty();
            let st = crate::output::Style {
                fancy,
                color: fancy && env.config.general.color && env.io.stderr_color_allowed(),
                unicode: env.io.unicode(),
            };
            if matches!(e, crate::error::Error::Aborted) {
                env.io.err(&crate::output::cancelled(st));
            } else {
                env.io.err(&crate::output::error(st, &e.to_string(), tail));
            }
            e.exit_code()
        }
    }
}
