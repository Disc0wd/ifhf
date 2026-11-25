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
    fn color_allowed(&self) -> bool;
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
        self.stdout_is_tty() && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
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
            let msg = format!("{e}");
            let tail = if matches!(e, crate::error::Error::Usage(_)) {
                "\n\nTry: ifhf --help"
            } else {
                ""
            };
            env.io.err(&format!("ifhf: {msg}{tail}\n"));
            e.exit_code()
        }
    }
}
