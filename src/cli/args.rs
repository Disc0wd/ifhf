//! Parses both command styles (`ifhf install x`, `ifhf -S x`) into one `Invocation`.
//! Hand-written on purpose: pacman-style clusters (`-Syu`) do not fit a derive parser, and it
//! keeps startup fast.

use crate::core::models::{Scope, is_valid_remote_name};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Install(Vec<String>),
    Remove(Vec<String>),
    Search(String),
    Info(Vec<String>),
    List,
    Updates,
    Update(Vec<String>),
    Sync,
    Remotes,
    Help,
    Version,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Opts {
    pub remote: Option<String>,
    pub scope: Option<Scope>,
    pub yes: bool,
    pub delete_data: bool,
    /// `list`/`remove`: include runtimes and other non-application refs.
    pub all: bool,
    pub no_color: bool,
    pub limit: Option<usize>,
    /// `list --search` / `-Qs`.
    pub filter: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub cmd: Command,
    pub opts: Opts,
}

fn usage<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Usage(msg.into()))
}

#[derive(Default)]
struct Pacman {
    op: Option<char>,
    s: bool,
    i: bool,
    u: bool,
    y: bool,
}

pub fn parse(args: &[String]) -> Result<Invocation> {
    let mut opts = Opts::default();
    let mut targets: Vec<String> = Vec::new();
    let mut verb: Option<String> = None;
    let mut pac: Option<Pacman> = None;
    let (mut help, mut version) = (false, false);
    let mut user = false;
    let mut system = false;
    let mut only_targets = false;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if only_targets || !arg.starts_with('-') || arg == "-" {
            if verb.is_none() && pac.is_none() && !only_targets {
                verb = Some(arg.clone());
            } else {
                targets.push(arg.clone());
            }
        } else if arg == "--" {
            only_targets = true;
        } else if let Some(long) = arg.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (long, None),
            };
            let mut value = |what: &str| -> Result<String> {
                match inline.clone().or_else(|| it.next().cloned()) {
                    Some(v) => Ok(v),
                    None => usage(format!("--{name} needs a value ({what}).")),
                }
            };
            match name {
                "remote" => {
                    let v = value("a remote name")?;
                    if !is_valid_remote_name(&v) {
                        return usage(format!("'{v}' is not a valid remote name."));
                    }
                    opts.remote = Some(v);
                }
                "limit" => {
                    let v = value("a number")?;
                    opts.limit = Some(
                        v.parse()
                            .or_else(|_| usage(format!("--limit needs a number, got '{v}'.")))?,
                    );
                }
                "search" => opts.filter = Some(value("a search term")?),
                "user" => user = true,
                "system" => system = true,
                "yes" | "noconfirm" => opts.yes = true,
                "delete-data" => opts.delete_data = true,
                "all" => opts.all = true,
                "no-color" => opts.no_color = true,
                "help" => help = true,
                "version" => version = true,
                _ => return usage(format!("Unknown option '--{name}'.")),
            }
        } else {
            let cluster = &arg[1..];
            let is_pacman =
                pac.is_some() || (verb.is_none() && cluster.starts_with(['S', 'R', 'Q']));
            for c in cluster.chars() {
                if is_pacman {
                    let p = pac.get_or_insert_with(Pacman::default);
                    match c {
                        'S' | 'R' | 'Q' => {
                            if p.op.is_some_and(|o| o != c) {
                                return usage("Only one of -S, -R, -Q may be used at a time.");
                            }
                            p.op = Some(c);
                        }
                        's' => p.s = true,
                        'i' => p.i = true,
                        'u' => p.u = true,
                        'y' => p.y = true,
                        _ => return usage(format!("Unknown option '-{c}'.")),
                    }
                } else {
                    match c {
                        'y' => opts.yes = true,
                        'h' => help = true,
                        'V' => version = true,
                        _ => return usage(format!("Unknown option '-{c}'.")),
                    }
                }
            }
        }
    }

    if user && system {
        return usage("--user and --system cannot be combined.");
    }
    opts.scope = if user {
        Some(Scope::User)
    } else if system {
        Some(Scope::System)
    } else {
        None
    };

    if help {
        return Ok(Invocation {
            cmd: Command::Help,
            opts,
        });
    }
    if version {
        return Ok(Invocation {
            cmd: Command::Version,
            opts,
        });
    }

    let cmd = match (pac, verb) {
        (Some(p), None) => pacman_command(p, targets, &mut opts)?,
        (Some(_), Some(_)) => unreachable!("a verb is never taken after a pacman flag"),
        (None, Some(v)) => natural_command(&v, targets)?,
        (None, None) => Command::Help,
    };
    Ok(Invocation { cmd, opts })
}

fn need_targets(name: &str, t: Vec<String>) -> Result<Vec<String>> {
    if t.is_empty() {
        usage(format!(
            "'{name}' needs at least one application name or ID."
        ))
    } else {
        Ok(t)
    }
}

fn no_targets(name: &str, t: &[String]) -> Result<()> {
    if t.is_empty() {
        Ok(())
    } else {
        usage(format!(
            "'{name}' does not take arguments (got '{}').",
            t[0]
        ))
    }
}

fn natural_command(verb: &str, t: Vec<String>) -> Result<Command> {
    Ok(match verb {
        "install" => Command::Install(need_targets(verb, t)?),
        "remove" => Command::Remove(need_targets(verb, t)?),
        "search" => Command::Search(need_targets(verb, t)?.join(" ")),
        "info" => Command::Info(need_targets(verb, t)?),
        "list" => {
            no_targets(verb, &t)?;
            Command::List
        }
        "updates" => {
            no_targets(verb, &t)?;
            Command::Updates
        }
        "update" => Command::Update(t),
        "sync" => {
            no_targets(verb, &t)?;
            Command::Sync
        }
        "remotes" => {
            no_targets(verb, &t)?;
            Command::Remotes
        }
        "help" => Command::Help,
        "version" => Command::Version,
        other => return usage(format!("Unknown command '{other}'. Try: ifhf --help")),
    })
}

fn pacman_command(p: Pacman, t: Vec<String>, opts: &mut Opts) -> Result<Command> {
    let flags = |extra: &str| {
        format!(
            "-{}{}{}{}{}",
            p.op.unwrap_or('?'),
            if p.y { "y" } else { "" },
            if p.s { "s" } else { "" },
            if p.i { "i" } else { "" },
            if p.u { "u" } else { "" }
        ) + extra
    };
    let unsupported = || {
        usage(format!(
            "Unsupported option combination '{}'. Try: ifhf --help",
            flags("")
        ))
    };
    match p.op {
        None => usage("Specify an operation: -S, -R or -Q."),
        Some('S') => match (p.y, p.s, p.i, p.u) {
            (false, false, false, false) => Ok(Command::Install(need_targets("-S", t)?)),
            (false, true, false, false) => Ok(Command::Search(need_targets("-Ss", t)?.join(" "))),
            (false, false, true, false) => Ok(Command::Info(need_targets("-Si", t)?)),
            (true, false, false, false) => {
                if t.is_empty() {
                    Ok(Command::Sync)
                } else {
                    usage(
                        "-Sy with applications is not supported: run 'ifhf -Sy' and then 'ifhf -S <app>'.",
                    )
                }
            }
            (_, false, false, true) => Ok(Command::Update(t)),
            _ => unsupported(),
        },
        Some('R') => {
            if p.s || p.i || p.u || p.y {
                return unsupported();
            }
            Ok(Command::Remove(need_targets("-R", t)?))
        }
        Some(_) => match (p.y, p.s, p.i, p.u) {
            (false, false, false, false) => {
                no_targets("-Q", &t)?;
                Ok(Command::List)
            }
            (false, true, false, false) => {
                opts.filter = Some(need_targets("-Qs", t)?.join(" "));
                Ok(Command::List)
            }
            (false, false, false, true) => {
                no_targets("-Qu", &t)?;
                Ok(Command::Updates)
            }
            (false, false, true, false) => Ok(Command::Info(need_targets("-Qi", t)?)),
            _ => unsupported(),
        },
    }
}

pub const HELP: &str = "\
ifhf - a friendly command-line frontend for Flatpak

USAGE
  ifhf <command> [options] [names...]      natural syntax
  ifhf -S|-R|-Q[flags] [names...]          pacman-style syntax (same behaviour)

COMMANDS                                   PACMAN
  install <app>...   Install by name or ID   -S <app>
  remove <app>...    Remove (keeps data)     -R <app>
  search <text>      Search all remotes      -Ss <text>
  info <app>...      Show details            -Si <app>
  list               List installed apps     -Q
  updates            List available updates  -Qu
  update [<app>...]  Apply updates           -Syu
  sync               Refresh metadata        -Sy
  remotes            Show configured remotes

OPTIONS
  --remote <name>      Only consider this remote
  --user, --system     Choose the installation (default: your config, normally user)
  --yes, --noconfirm   Do not ask for confirmation (required when not on a terminal)
  --delete-data        With remove: also delete the application's user data
  --all                With list/remove: include runtimes and other refs
  --search <text>      With list: filter installed applications
  --limit <n>          With search: show at most n results (default 25, 0 = all)
  --no-color           Disable colour (NO_COLOR is also respected)
  -h, --help           Show this help
  -V, --version        Show the version

EXIT STATUS
  0 success   1 failure   2 usage error   3 not found
  4 ambiguous 5 confirmation required (use --yes)   6 cancelled
";

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &str) -> Result<Invocation> {
        parse(&a.split_whitespace().map(String::from).collect::<Vec<_>>())
    }
    fn cmd(a: &str) -> Command {
        p(a).unwrap().cmd
    }
    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn natural_and_compact_are_equivalent() {
        for (n, c) in [
            ("install spotify", "-S spotify"),
            ("remove spotify", "-R spotify"),
            ("search firefox", "-Ss firefox"),
            ("info firefox", "-Si firefox"),
            ("list", "-Q"),
            ("updates", "-Qu"),
            ("update", "-Syu"),
            ("sync", "-Sy"),
        ] {
            assert_eq!(p(n).unwrap(), p(c).unwrap(), "{n} vs {c}");
        }
        assert_eq!(
            cmd("install spotify discord"),
            Command::Install(v(&["spotify", "discord"]))
        );
        assert_eq!(
            cmd("-S spotify discord firefox"),
            Command::Install(v(&["spotify", "discord", "firefox"]))
        );
    }

    #[test]
    fn pacman_variants() {
        assert_eq!(cmd("-Su"), Command::Update(vec![]));
        assert_eq!(
            cmd("-Ss web browser"),
            Command::Search("web browser".into())
        );
        let i = p("-Qs fire").unwrap();
        assert_eq!(i.cmd, Command::List);
        assert_eq!(i.opts.filter.as_deref(), Some("fire"));
        assert_eq!(cmd("-Qi firefox"), Command::Info(v(&["firefox"])));
        assert_eq!(cmd("-S -s x"), Command::Search("x".into()));
    }

    #[test]
    fn options_in_either_position() {
        let i = p("install firefox --remote flathub --user --yes").unwrap();
        assert_eq!(i.opts.remote.as_deref(), Some("flathub"));
        assert_eq!(i.opts.scope, Some(Scope::User));
        assert!(i.opts.yes);
        let i = p("--system --remote=fedora -S firefox").unwrap();
        assert_eq!(i.opts.remote.as_deref(), Some("fedora"));
        assert_eq!(i.opts.scope, Some(Scope::System));
        assert_eq!(i.cmd, Command::Install(v(&["firefox"])));
        assert!(p("remove x --delete-data").unwrap().opts.delete_data);
        assert!(p("list --all --search x").unwrap().opts.all);
        assert_eq!(p("search x --limit 5").unwrap().opts.limit, Some(5));
        assert!(p("install x -y").unwrap().opts.yes);
        assert!(p("-S x --noconfirm").unwrap().opts.yes);
    }

    #[test]
    fn usage_errors() {
        for bad in [
            "install",
            "remove",
            "frobnicate",
            "list extra",
            "--bogus",
            "install x --user --system",
            "install x --remote",
            "install x --remote --bad",
            "search x --limit abc",
            "-Sx",
            "-SR x",
            "-Sy x",
            "-Ss",
            "-R",
            "-Qx",
            "-Ry x",
            "-Qu x",
        ] {
            let e = p(bad).expect_err(bad);
            assert_eq!(e.exit_code(), 2, "{bad}");
        }
    }

    #[test]
    fn help_version_and_empty() {
        assert_eq!(cmd(""), Command::Help);
        assert_eq!(cmd("--help"), Command::Help);
        assert_eq!(cmd("install x -h"), Command::Help);
        assert_eq!(cmd("-V"), Command::Version);
        assert_eq!(cmd("version"), Command::Version);
    }

    #[test]
    fn double_dash_makes_the_rest_targets() {
        assert_eq!(cmd("install -- -weird"), Command::Install(v(&["-weird"])));
    }

    #[test]
    fn query_with_spaces_arrives_intact() {
        let args = vec!["install".to_string(), "visual studio code".to_string()];
        assert_eq!(
            parse(&args).unwrap().cmd,
            Command::Install(v(&["visual studio code"]))
        );
    }
}
