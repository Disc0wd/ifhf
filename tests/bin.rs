//! Runs the real `ifhf` binary against a stub flatpak and temporary XDG/Flatpak directories.
mod common;
use common::*;
use std::process::{Command, Output, Stdio};

struct Rig {
    stub: Stub,
    home: tempfile::TempDir,
}

impl Rig {
    fn new() -> Rig {
        let home = tempfile::tempdir().unwrap();
        write_metadata(&home.path().join("system"), "flathub", &flathub_xml());
        write_metadata(&home.path().join("system"), "fedora", &fedora_xml());
        Rig {
            stub: Stub::new(),
            home,
        }
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let h = self.home.path();
        let mut c = Command::new(env!("CARGO_BIN_EXE_ifhf"));
        c.args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", h)
            .env("XDG_CACHE_HOME", h.join("cache"))
            .env("XDG_CONFIG_HOME", h.join("config"))
            .env("FLATPAK_USER_DIR", h.join("user"))
            .env("FLATPAK_SYSTEM_DIR", h.join("system"))
            .env("IFHF_FLATPAK", self.stub.path())
            .stdin(Stdio::null());
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn search_works_end_to_end_and_builds_the_index() {
    let r = Rig::new();
    let o = r.run(&["search", "spotify"]);
    assert!(o.status.success(), "{}", s(&o.stderr));
    assert!(s(&o.stdout).starts_with("Spotify\n  com.spotify.Client"));
    assert!(r.home.path().join("cache/ifhf/index.tsv").exists());
    assert!(
        r.stub.log().iter().any(|l| l.starts_with("remotes")),
        "first run discovers remotes via flatpak"
    );
    let before = r.stub.log().len();
    let o2 = r.run(&["-Ss", "spotify"]);
    assert_eq!(s(&o.stdout), s(&o2.stdout));
    assert_eq!(
        r.stub.log().len(),
        before,
        "warm search must not spawn flatpak at all"
    );
}

#[test]
fn list_and_compact_list_match() {
    let r = Rig::new();
    let a = r.run(&["list"]);
    let b = r.run(&["-Q"]);
    assert!(a.status.success());
    assert_eq!(s(&a.stdout), s(&b.stdout));
    assert_eq!(
        s(&a.stdout),
        "Firefox\torg.mozilla.firefox\tstable\tuser\tflathub\nSpotify\tcom.spotify.Client\tstable\tsystem\tflathub\n"
    );
}

#[test]
fn install_needs_yes_without_a_terminal_then_runs_flatpak() {
    let r = Rig::new();
    let o = r.run(&["install", "discord"]);
    assert_eq!(o.status.code(), Some(5), "{}", s(&o.stderr));
    assert!(!r.stub.log().iter().any(|l| l.starts_with("install")));
    let o = r.run(&["-S", "discord", "--yes"]);
    assert_eq!(o.status.code(), Some(0), "{}", s(&o.stderr));
    assert!(
        r.stub.log().contains(
            &"install --system -y flathub app/com.discordapp.Discord/x86_64/stable".to_string()
        ),
        "{:?}",
        r.stub.log()
    );
}

#[test]
fn remove_keeps_data_unless_asked() {
    let r = Rig::new();
    assert_eq!(
        r.run(&["remove", "spotify", "--yes"]).status.code(),
        Some(0)
    );
    assert_eq!(
        r.run(&["-R", "spotify", "--yes", "--delete-data"])
            .status
            .code(),
        Some(0)
    );
    let log = r.stub.log();
    assert!(
        log.contains(&"uninstall --system -y com.spotify.Client".to_string()),
        "{log:?}"
    );
    assert!(
        log.contains(&"uninstall --system -y --delete-data com.spotify.Client".to_string()),
        "{log:?}"
    );
}

#[test]
fn flatpak_failure_gives_actionable_error_and_nonzero_exit() {
    let r = Rig::new();
    r.stub.set("fail", "");
    let o = r.run(&["install", "discord", "--yes"]);
    assert_eq!(o.status.code(), Some(1));
    let e = s(&o.stderr);
    assert!(
        e.contains("Could not install Discord") && e.contains("ifhf remotes"),
        "{e}"
    );
}

#[test]
fn missing_flatpak_executable() {
    let r = Rig::new();
    let mut c = r.cmd(&["list"]);
    c.env("IFHF_FLATPAK", "/no/such/flatpak");
    let o = c.output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(s(&o.stderr).contains("Flatpak was not found"));
}

#[test]
fn invalid_config_is_reported_with_the_path() {
    let r = Rig::new();
    let dir = r.home.path().join("config/ifhf");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[general]\ndefault_scope = \"moon\"\n",
    )
    .unwrap();
    let o = r.run(&["list"]);
    assert_eq!(o.status.code(), Some(1));
    let e = s(&o.stderr);
    assert!(
        e.contains("config.toml") && e.contains("default_scope"),
        "{e}"
    );
}

#[test]
fn config_file_is_honoured() {
    let r = Rig::new();
    let dir = r.home.path().join("config/ifhf");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[general]\nconfirm_install = false\n",
    )
    .unwrap();
    assert_eq!(r.run(&["install", "discord"]).status.code(), Some(0));
}

#[test]
fn exit_codes() {
    let r = Rig::new();
    assert_eq!(r.run(&["bogus"]).status.code(), Some(2));
    assert_eq!(r.run(&["search", "zzqqnothing"]).status.code(), Some(3));
    assert_eq!(
        r.run(&["install", "firefox", "--yes"]).status.code(),
        Some(4)
    );
    assert_eq!(r.run(&["--version"]).status.code(), Some(0));
    assert_eq!(r.run(&[]).status.code(), Some(0));
}

#[test]
fn updates_end_to_end() {
    let r = Rig::new();
    r.stub.set(
        "updates_system.tsv",
        "app/com.spotify.Client/x86_64/stable\tSpotify\tflathub\n",
    );
    let o = r.run(&["-Qu"]);
    assert!(o.status.success(), "{}", s(&o.stderr));
    assert_eq!(
        s(&o.stdout),
        "Spotify\tcom.spotify.Client\tstable\tsystem\tflathub\tapp\n"
    );
    assert_eq!(r.run(&["-Syu"]).status.code(), Some(5));
    assert_eq!(r.run(&["-Syu", "--yes"]).status.code(), Some(0));
    assert!(r.stub.log().contains(&"update --system -y".to_string()));
}

#[test]
fn no_color_when_not_a_terminal() {
    let r = Rig::new();
    let o = r.run(&["search", "spotify"]);
    assert!(!s(&o.stdout).contains('\x1b'));
}
