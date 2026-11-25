#![allow(dead_code)]
use ifhf::backend::MockBackend;
use ifhf::backend::traits::host_arch;
use ifhf::cli::{self, Env, Io};
use ifhf::config::Config;
use ifhf::core::models::{Remote, Scope};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

pub struct TestIo {
    pub out: String,
    pub err: String,
    pub answers: VecDeque<String>,
    pub asked: Vec<String>,
    pub interactive: bool,
    pub tty: bool,
}

impl TestIo {
    pub fn non_interactive() -> Self {
        TestIo {
            out: String::new(),
            err: String::new(),
            answers: VecDeque::new(),
            asked: vec![],
            interactive: false,
            tty: false,
        }
    }
    pub fn answering(a: &[&str]) -> Self {
        TestIo {
            interactive: true,
            answers: a.iter().map(|s| s.to_string()).collect(),
            ..Self::non_interactive()
        }
    }
}

impl Io for TestIo {
    fn out(&mut self, s: &str) {
        self.out.push_str(s);
    }
    fn err(&mut self, s: &str) {
        self.err.push_str(s);
    }
    fn ask(&mut self, prompt: &str) -> Option<String> {
        self.asked.push(prompt.to_string());
        self.answers.pop_front()
    }
    fn interactive(&self) -> bool {
        self.interactive
    }
    fn stdout_is_tty(&self) -> bool {
        self.tty
    }
    fn color_allowed(&self) -> bool {
        false
    }
}

fn component(id: &str, name: &str, summary: &str, kw: &[&str], desc: &str) -> String {
    let kws: String = kw
        .iter()
        .map(|k| format!("<keyword>{k}</keyword>"))
        .collect();
    format!(
        r#"<component type="desktop-application"><id>{id}</id><name>{name}</name><summary>{summary}</summary><description><p>{desc}</p></description><keywords>{kws}</keywords><bundle type="flatpak">app/{id}/{arch}/stable</bundle></component>"#,
        arch = host_arch()
    )
}

pub fn flathub_xml() -> String {
    let c = [
        component(
            "org.mozilla.firefox",
            "Firefox",
            "Fast, private web browser",
            &["Browser", "Internet"],
            "A browser.",
        ),
        component(
            "org.chromium.Chromium",
            "Chromium",
            "Open-source web browser",
            &[],
            "Another browser.",
        ),
        component(
            "com.spotify.Client",
            "Spotify",
            "Music streaming",
            &[],
            "Stream music.",
        ),
        component(
            "org.x.spotify_tui",
            "Spotify TUI",
            "Terminal client for Spotify",
            &[],
            "",
        ),
        component(
            "org.gnome.Console",
            "GNOME Console",
            "A simple terminal",
            &[],
            "",
        ),
        component(
            "org.gnome.Ptyxis",
            "Ptyxis",
            "Container-oriented terminal",
            &[],
            "",
        ),
        component(
            "org.d.Docs",
            "Docs",
            "Documents",
            &[],
            "Edits PDF files with annotations",
        ),
        component("com.discordapp.Discord", "Discord", "Chat", &[], ""),
    ];
    format!(
        r#"<?xml version="1.0"?><components version="0.8">{}</components>"#,
        c.concat()
    )
}

pub fn fedora_xml() -> String {
    let c = [component(
        "org.mozilla.Firefox",
        "Firefox",
        "Fedora's Firefox",
        &[],
        "",
    )];
    format!(r#"<components version="0.8">{}</components>"#, c.concat())
}

pub fn write_metadata(base: &Path, remote: &str, xml: &str) {
    let d = base
        .join("appstream")
        .join(remote)
        .join(host_arch())
        .join("active");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("appstream.xml"), xml).unwrap();
}

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub backend: MockBackend,
    pub cache: PathBuf,
    pub config: Config,
}

impl Fixture {
    /// System installation with remotes flathub and fedora; no user remotes.
    pub fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let system = dir.path().join("system");
        let user = dir.path().join("user");
        std::fs::create_dir_all(&user).unwrap();
        write_metadata(&system, "flathub", &flathub_xml());
        write_metadata(&system, "fedora", &fedora_xml());
        let remote = |name: &str, scope| Remote {
            name: name.into(),
            url: format!("https://{name}"),
            scope,
            enabled: true,
        };
        let backend = MockBackend {
            dirs: vec![(Scope::User, user), (Scope::System, system)],
            remotes: vec![
                remote("flathub", Scope::System),
                remote("fedora", Scope::System),
            ],
            ..MockBackend::default()
        };
        let cache = dir.path().join("cache");
        Fixture {
            dir,
            backend,
            cache,
            config: Config::default(),
        }
    }

    pub fn system_dir(&self) -> PathBuf {
        self.dir.path().join("system")
    }

    pub fn run(&self, io: &mut TestIo, args: &str) -> i32 {
        let args: Vec<String> = args.split_whitespace().map(String::from).collect();
        self.run_args(io, &args)
    }

    pub fn run_args(&self, io: &mut TestIo, args: &[String]) -> i32 {
        let mut env = Env {
            backend: &self.backend,
            config: &self.config,
            cache_dir: Some(self.cache.clone()),
            io,
        };
        cli::run(args, &mut env)
    }
}

/// A fake `flatpak` executable. It logs every invocation (args joined by spaces) to `log`
/// and prints canned data from files next to it; a `fail` file makes mutating commands fail.
pub struct Stub {
    pub dir: tempfile::TempDir,
}

pub const INSTALLED: &str = "com.spotify.Client/x86_64/stable\tSpotify\tflathub\tsystem\tsystem,current\n\
org.mozilla.firefox/x86_64/stable\tFirefox\tflathub\tuser\tuser,current\n\
org.freedesktop.Platform/x86_64/25.08\tFreedesktop Platform\tflathub\tsystem\tsystem,runtime\n";
pub const REMOTES: &str = "fedora\tsystem,oci\toci+https://registry.fedoraproject.org\n\
fedora-testing\tsystem,disabled,oci\toci+https://x\n\
flathub\tsystem\thttps://dl.flathub.org/repo/\n";

impl Stub {
    pub fn new() -> Stub {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n[ \"$1\" = --probe ] && exit 0\nd=\"$(dirname \"$0\")\"\necho \"$*\" >> \"$d/log\"\n\
case \"$1\" in\n\
 list) cat \"$d/installed.tsv\";;\n\
 remotes) cat \"$d/remotes.tsv\";;\n\
 remote-ls) case \"$*\" in *--user*) cat \"$d/updates_user.tsv\" 2>/dev/null;; *) cat \"$d/updates_system.tsv\" 2>/dev/null;; esac;;\n\
 install|uninstall|update) if [ -f \"$d/fail\" ]; then echo \"boom: remote unreachable\" >&2; exit 1; fi;;\n\
esac\nexit 0\n";
        let p = dir.path().join("flatpak");
        std::fs::write(&p, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Another test thread may fork while our script's write fd is open, which makes the first
        // exec fail with ETXTBSY. Probe until it is executable so tests never see that race.
        for _ in 0..500 {
            match std::process::Command::new(&p).arg("--probe").status() {
                Err(e) if e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                _ => break,
            }
        }
        let s = Stub { dir };
        s.set("installed.tsv", INSTALLED);
        s.set("remotes.tsv", REMOTES);
        s
    }
    pub fn path(&self) -> PathBuf {
        self.dir.path().join("flatpak")
    }
    pub fn set(&self, file: &str, content: &str) {
        std::fs::write(self.dir.path().join(file), content).unwrap();
    }
    pub fn log(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }
}
