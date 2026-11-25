mod common;
use common::*;
use ifhf::backend::{Backend, FlatpakCli};
use ifhf::core::models::Scope;

#[test]
fn parses_installed_applications_and_runtimes() {
    let stub = Stub::new();
    let b = FlatpakCli::new(stub.path());
    let l = b.installed().unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(l[0].id, "com.spotify.Client");
    assert_eq!(l[0].name, "Spotify");
    assert_eq!(
        (
            l[0].branch.as_str(),
            l[0].arch.as_str(),
            l[0].origin.as_str()
        ),
        ("stable", "x86_64", "flathub")
    );
    assert_eq!(l[0].installation, "system");
    assert!(!l[0].is_runtime);
    assert_eq!(l[1].scope(), Scope::User);
    assert!(l[2].is_runtime);
    assert_eq!(
        stub.log(),
        vec!["list --columns=ref,name,origin,installation,options"]
    );
}

#[test]
fn tolerates_malformed_lines_and_empty_output() {
    let stub = Stub::new();
    stub.set(
        "installed.tsv",
        "garbage\n\nonly\ttwo\nok.app.Id/x86_64/stable\t\tflathub\tsystem\tsystem\n",
    );
    let l = FlatpakCli::new(stub.path()).installed().unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].name, "");
    stub.set("installed.tsv", "");
    assert!(FlatpakCli::new(stub.path()).installed().unwrap().is_empty());
}

#[test]
fn parses_remotes_including_disabled_and_oci() {
    let stub = Stub::new();
    let r = FlatpakCli::new(stub.path()).remotes().unwrap();
    assert_eq!(r.len(), 3);
    assert!(r[0].enabled && r[0].scope == Scope::System && r[0].name == "fedora");
    assert!(!r[1].enabled);
    assert_eq!(r[2].url, "https://dl.flathub.org/repo/");
    assert_eq!(
        stub.log(),
        vec!["remotes --show-disabled --columns=name,options,url"]
    );
}

#[test]
fn updates_query_each_installation_once() {
    let stub = Stub::new();
    stub.set("updates_system.tsv", "app/com.spotify.Client/x86_64/stable\tSpotify\tflathub\nruntime/org.gnome.Platform/x86_64/51\tGNOME\tflathub\n");
    let u = FlatpakCli::new(stub.path()).updates().unwrap();
    assert_eq!(u.len(), 2);
    assert_eq!(u[0].id, "com.spotify.Client");
    assert_eq!(u[0].installation, "system");
    assert!(!u[0].is_runtime && u[1].is_runtime);
    assert_eq!(
        stub.log(),
        vec![
            "remote-ls --updates --user --columns=ref,name,origin",
            "remote-ls --updates --system --columns=ref,name,origin"
        ],
        "no extra `flatpak remotes` launch"
    );
}

#[test]
fn mutating_commands_pass_separate_arguments_and_assume_yes() {
    let stub = Stub::new();
    let b = FlatpakCli::new(stub.path());
    b.install(
        Scope::User,
        "flathub",
        &[
            "app/com.spotify.Client/x86_64/stable".into(),
            "org.a.B".into(),
        ],
    )
    .unwrap();
    b.remove("system", &["com.spotify.Client".into()], false)
        .unwrap();
    b.remove("user", &["org.a.B".into()], true).unwrap();
    b.update("system", &[]).unwrap();
    b.update("user", &["org.a.B".into()]).unwrap();
    b.sync().unwrap();
    assert_eq!(
        stub.log(),
        vec![
            "install --user -y flathub app/com.spotify.Client/x86_64/stable org.a.B",
            "uninstall --system -y com.spotify.Client",
            "uninstall --user -y --delete-data org.a.B",
            "update --system -y",
            "update --user -y org.a.B",
            "update --appstream",
        ]
    );
}

#[test]
fn failures_surface_the_exit_status() {
    let stub = Stub::new();
    stub.set("fail", "");
    let e = FlatpakCli::new(stub.path())
        .install(Scope::User, "flathub", &["org.a.B".into()])
        .unwrap_err();
    assert!(e.to_string().contains("exited with"), "{e}");
    assert_eq!(e.exit_code(), 1);
}

#[test]
fn invalid_input_never_reaches_flatpak() {
    let stub = Stub::new();
    let b = FlatpakCli::new(stub.path());
    assert!(
        b.install(Scope::User, "flathub", &["--system".into()])
            .is_err()
    );
    assert!(
        b.install(Scope::User, "$(touch pwned)", &["org.a.B".into()])
            .is_err()
    );
    assert!(b.remove("user", &["org.a.B;reboot".into()], false).is_err());
    assert!(b.update("user", &["-v".into()]).is_err());
    assert!(
        stub.log().is_empty(),
        "flatpak must not have been invoked: {:?}",
        stub.log()
    );
}

#[test]
fn missing_flatpak_is_a_clear_error_everywhere() {
    let b = FlatpakCli::new("/definitely/not/flatpak");
    for e in [
        b.installed().err().unwrap(),
        b.remotes().err().unwrap(),
        b.updates().err().unwrap(),
        b.install(Scope::User, "flathub", &["org.a.B".into()])
            .err()
            .unwrap(),
        b.sync().err().unwrap(),
    ] {
        assert!(e.to_string().contains("Flatpak was not found"), "{e}");
    }
}

#[test]
fn discovers_appstream_files_for_enabled_remotes_only() {
    let stub = Stub::new();
    let f = Fixture::new();
    // Real FlatpakCli discovery with the fixture's installation directories.
    struct WithDirs(FlatpakCli, Vec<(Scope, std::path::PathBuf)>);
    impl Backend for WithDirs {
        fn installation_dirs(&self) -> Vec<(Scope, std::path::PathBuf)> {
            self.1.clone()
        }
        fn remotes(&self) -> ifhf::error::Result<Vec<ifhf::core::models::Remote>> {
            self.0.remotes()
        }
        fn installed(&self) -> ifhf::error::Result<Vec<ifhf::core::models::Installed>> {
            self.0.installed()
        }
        fn install(&self, a: Scope, b: &str, c: &[String]) -> ifhf::error::Result<()> {
            self.0.install(a, b, c)
        }
        fn remove(&self, a: &str, b: &[String], c: bool) -> ifhf::error::Result<()> {
            self.0.remove(a, b, c)
        }
        fn updates(&self) -> ifhf::error::Result<Vec<ifhf::core::models::Update>> {
            self.0.updates()
        }
        fn update(&self, a: &str, b: &[String]) -> ifhf::error::Result<()> {
            self.0.update(a, b)
        }
        fn sync(&self) -> ifhf::error::Result<()> {
            self.0.sync()
        }
    }
    let b = WithDirs(FlatpakCli::new(stub.path()), f.backend.dirs.clone());
    let src = b.appstream_sources().unwrap();
    let names: Vec<_> = src.iter().map(|s| s.remote.as_str()).collect();
    assert_eq!(names, vec!["fedora", "flathub"]); // fedora-testing is disabled and has no files
    assert!(
        src.iter()
            .all(|s| s.scope == Scope::System && s.path.ends_with("appstream.xml"))
    );
}
