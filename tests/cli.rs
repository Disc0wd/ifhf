mod common;
use common::*;
use ifhf::config::Config;
use ifhf::core::models::{Remote, Scope, Update};

fn yes(f: &Fixture, args: &str) -> (i32, TestIo) {
    let mut io = TestIo::non_interactive();
    let code = f.run(&mut io, &format!("{args} --yes"));
    (code, io)
}

// ---- install -------------------------------------------------------------------------

#[test]
fn install_by_name_after_confirmation() {
    let f = Fixture::new();
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(f.run(&mut io, "install spotify"), 0, "{}", io.err);
    assert_eq!(
        f.backend.calls(),
        vec!["install system flathub app/com.spotify.Client/x86_64/stable"]
    );
    assert!(io.out.contains("Application: com.spotify.Client"));
    assert!(io.out.contains("Remote:      flathub"));
    assert!(io.out.contains(
        "Scope:       system (remote 'flathub' is not configured for the user installation)"
    ));
    assert_eq!(io.asked, vec!["Install Spotify? [Y/n] "]);
}

#[test]
fn default_answer_is_yes_and_no_cancels() {
    let f = Fixture::new();
    let mut io = TestIo::answering(&[""]);
    assert_eq!(f.run(&mut io, "install spotify"), 0);
    assert_eq!(f.backend.calls().len(), 1);

    let f = Fixture::new();
    let mut io = TestIo::answering(&["n"]);
    assert_eq!(f.run(&mut io, "install spotify"), 6);
    assert!(f.backend.calls().is_empty());
}

#[test]
fn natural_and_compact_syntax_do_the_same() {
    let a = Fixture::new();
    let b = Fixture::new();
    let (ca, ioa) = yes(&a, "install spotify");
    let (cb, iob) = yes(&b, "-S spotify");
    assert_eq!((ca, cb), (0, 0));
    assert_eq!(a.backend.calls(), b.backend.calls());
    assert_eq!(ioa.out, iob.out);
}

#[test]
fn install_by_exact_id_beats_name_matches() {
    let f = Fixture::new();
    let (code, _) = yes(&f, "install org.mozilla.firefox");
    assert_eq!(code, 0);
    assert_eq!(
        f.backend.calls(),
        vec!["install system flathub app/org.mozilla.firefox/x86_64/stable"]
    );
}

#[test]
fn several_apps_one_confirmation_one_transaction() {
    let f = Fixture::new();
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(
        f.run(&mut io, "install spotify discord chromium"),
        0,
        "{}",
        io.err
    );
    assert_eq!(io.asked.len(), 1);
    assert_eq!(io.asked[0], "Install 3 applications? [Y/n] ");
    assert_eq!(f.backend.calls().len(), 1);
    let call = &f.backend.calls()[0];
    assert!(call.starts_with("install system flathub "));
    for id in [
        "com.spotify.Client",
        "com.discordapp.Discord",
        "org.chromium.Chromium",
    ] {
        assert!(call.contains(id), "{call}");
    }
}

#[test]
fn nothing_runs_if_any_requested_app_is_unknown() {
    let f = Fixture::new();
    let (code, io) = yes(&f, "install spotify zzqqnothing");
    assert_eq!(code, 3);
    assert!(f.backend.calls().is_empty());
    assert!(io.err.contains("No application matches 'zzqqnothing'"));
}

#[test]
fn same_app_twice_is_installed_once() {
    let f = Fixture::new();
    let (code, _) = yes(&f, "install spotify com.spotify.Client");
    assert_eq!(code, 0);
    assert_eq!(f.backend.calls().len(), 1);
    assert_eq!(
        f.backend.calls()[0].matches("com.spotify.Client").count(),
        1
    );
}

#[test]
fn ambiguity_is_never_resolved_silently() {
    let f = Fixture::new();
    for args in ["install firefox --yes", "install terminal --yes"] {
        let mut io = TestIo::non_interactive();
        assert_eq!(f.run(&mut io, args), 4, "{args}");
        assert!(io.err.contains("Multiple applications match"), "{}", io.err);
        assert!(f.backend.calls().is_empty());
    }
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "install firefox --yes");
    assert!(io.err.contains("org.mozilla.firefox") && io.err.contains("org.mozilla.Firefox"));
    assert!(io.err.contains("fedora") && io.err.contains("flathub"));
}

#[test]
fn interactive_ambiguity_lets_the_user_choose() {
    let f = Fixture::new();
    // candidates sort by name then id: org.mozilla.Firefox (fedora) < org.mozilla.firefox (flathub)
    let mut io = TestIo::answering(&["2", "y"]);
    assert_eq!(f.run(&mut io, "install firefox"), 0, "{}", io.err);
    assert!(io.asked[0].starts_with("Choose a result [1-2"));
    let call = &f.backend.calls()[0];
    assert!(
        call.contains("org.mozilla.firefox") && call.contains("flathub"),
        "{call}"
    );
    let f = Fixture::new();
    let mut io = TestIo::answering(&["q"]);
    assert_eq!(f.run(&mut io, "install firefox"), 6);
    assert!(f.backend.calls().is_empty());
    let f = Fixture::new();
    let mut io = TestIo::answering(&["9", "x", "7"]);
    assert_eq!(
        f.run(&mut io, "install firefox"),
        6,
        "three invalid choices cancel"
    );
}

#[test]
fn remote_option_disambiguates() {
    let f = Fixture::new();
    let (code, _) = yes(&f, "install firefox --remote fedora");
    assert_eq!(code, 0);
    assert_eq!(
        f.backend.calls(),
        vec!["install system fedora app/org.mozilla.Firefox/x86_64/stable"]
    );
    let f = Fixture::new();
    let (code, io) = yes(&f, "install spotify --remote fedora");
    assert_eq!(code, 3);
    assert!(io.err.contains("in remote 'fedora'"));
}

#[test]
fn fuzzy_only_match_needs_an_explicit_choice_under_yes() {
    let f = Fixture::new();
    // "spot" only prefix-matches (Spotify and Spotify TUI): ambiguous. "romiu" only fuzzy matches Chromium.
    let (code, io) = yes(&f, "install romiu");
    assert_eq!(code, 4, "{}", io.err);
    assert!(io.err.contains("not an exact match"), "{}", io.err);
    assert!(f.backend.calls().is_empty());
    // Interactive user without --yes sees the plan and confirms.
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(f.run(&mut io, "install romiu"), 0);
    assert!(f.backend.calls()[0].contains("Chromium"));
}

#[test]
fn non_interactive_requires_yes() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "install spotify"), 5);
    assert!(io.err.contains("--yes"));
    assert!(f.backend.calls().is_empty());
    assert!(io.asked.is_empty(), "must not wait for input");
    assert_eq!(yes(&f, "install spotify").0, 0);
}

#[test]
fn user_scope_prefers_user_remotes_and_explicit_scope_is_strict() {
    let f = Fixture::new();
    write_metadata(&f.dir.path().join("user"), "flathub", &flathub_xml());
    let mut f = f;
    f.backend.remotes.push(Remote {
        name: "flathub".into(),
        url: "u".into(),
        scope: Scope::User,
        enabled: true,
    });
    // default scope (user) wins when the remote exists there
    let (code, io) = yes(&f, "install spotify");
    assert_eq!(code, 0, "{}", io.err);
    assert_eq!(
        f.backend.calls(),
        vec!["install user flathub app/com.spotify.Client/x86_64/stable"]
    );
    assert!(!io.out.contains("not configured"));
    // explicit --system
    let (code, _) = yes(&f, "install discord --system");
    assert_eq!(code, 0);
    assert_eq!(
        f.backend.calls()[1],
        "install system flathub app/com.discordapp.Discord/x86_64/stable"
    );
    // explicit --user with a system-only remote is NotFound, not a silent fallback
    let g = Fixture::new();
    let (code, _) = yes(&g, "install spotify --user");
    assert_eq!(code, 3);
    assert!(g.backend.calls().is_empty());
}

#[test]
fn config_default_scope_and_confirmation_switches() {
    let mut f = Fixture::new();
    f.config = Config::parse(
        "[general]\nconfirm_install = false\ndefault_scope = \"system\"\n",
        "t",
    )
    .unwrap();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "install spotify"), 0, "{}", io.err);
    assert!(io.asked.is_empty());
    assert!(!io.out.contains("not configured"), "{}", io.out);
}

#[test]
fn config_default_remote_is_overridden_by_flag() {
    let mut f = Fixture::new();
    f.config = Config::parse("[general]\ndefault_remote = \"fedora\"\n", "t").unwrap();
    assert_eq!(yes(&f, "install firefox").0, 0);
    assert!(f.backend.calls()[0].contains("fedora"));
    assert_eq!(yes(&f, "install firefox --remote flathub").0, 0);
    assert!(f.backend.calls()[1].contains("flathub"));
}

#[test]
fn partial_failure_reports_exactly_what_happened() {
    let mut f = Fixture::new();
    f.backend.fail_on = vec!["Discord".into()];
    let (code, io) = yes(&f, "install spotify discord");
    assert_eq!(code, 1);
    assert!(io.err.contains("Succeeded: Spotify"), "{}", io.err);
    assert!(io.err.contains("Failed: Discord"), "{}", io.err);
    assert!(
        io.err.contains("ifhf remotes"),
        "actionable next step: {}",
        io.err
    );
}

#[test]
fn total_failure_has_a_useful_message() {
    let mut f = Fixture::new();
    f.backend.fail_on = vec!["com.spotify".into()];
    let (code, io) = yes(&f, "install spotify");
    assert_eq!(code, 1);
    assert!(io.err.contains("Could not install Spotify"), "{}", io.err);
    assert!(
        io.err.contains("mock: cannot install"),
        "keeps flatpak's error: {}",
        io.err
    );
    assert!(io.err.contains("ifhf remotes"));
}

// ---- remove --------------------------------------------------------------------------

#[test]
fn remove_keeps_user_data_by_default() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(f.run(&mut io, "remove spotify"), 0, "{}", io.err);
    assert_eq!(f.backend.calls(), vec!["remove system com.spotify.Client"]);
    assert!(io.out.contains("Application data is kept"));
    assert!(!io.out.contains("WARNING"));
}

#[test]
fn compact_remove_equals_natural() {
    let a = Fixture::new();
    let b = Fixture::new();
    for f in [&a, &b] {
        f.backend
            .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    }
    assert_eq!(yes(&a, "remove spotify").0, 0);
    assert_eq!(yes(&b, "-R spotify").0, 0);
    assert_eq!(a.backend.calls(), b.backend.calls());
}

#[test]
fn delete_data_is_explicit_warned_and_defaults_to_no() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let mut io = TestIo::answering(&[""]);
    assert_eq!(
        f.run(&mut io, "remove spotify --delete-data"),
        6,
        "empty answer must not delete data"
    );
    assert!(io.out.contains("WARNING") && io.out.contains("permanently deletes"));
    assert!(io.asked[0].contains("[y/N]"));
    assert!(f.backend.calls().is_empty());
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(f.run(&mut io, "remove spotify --delete-data"), 0);
    assert_eq!(
        f.backend.calls(),
        vec!["remove system --delete-data com.spotify.Client"]
    );
}

#[test]
fn config_cannot_skip_the_delete_data_confirmation() {
    let mut f = Fixture::new();
    f.config = Config::parse("[general]\nconfirm_remove = false\n", "t").unwrap();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "remove spotify --delete-data"), 5);
    assert!(f.backend.calls().is_empty());
    let mut io = TestIo::non_interactive();
    assert_eq!(
        f.run(&mut io, "remove spotify"),
        0,
        "plain remove honours confirm_remove=false"
    );
}

#[test]
fn remove_ambiguity_requires_selection() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "user");
    let (code, io) = yes(&f, "remove spotify");
    assert_eq!(code, 4);
    assert!(
        io.err.contains("(user)") && io.err.contains("(system)"),
        "{}",
        io.err
    );
    assert!(f.backend.calls().is_empty());
    let (code, _) = yes(&f, "remove spotify --user");
    assert_eq!(code, 0);
    assert_eq!(f.backend.calls(), vec!["remove user com.spotify.Client"]);
}

#[test]
fn remove_unknown_app_and_runtimes_are_not_candidates() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    f.backend
        .installed
        .lock()
        .unwrap()
        .push(ifhf::core::models::Installed {
            id: "org.freedesktop.Platform".into(),
            name: "Freedesktop Platform".into(),
            branch: "25.08".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            installation: "system".into(),
            is_runtime: true,
        });
    let (code, io) = yes(&f, "remove nothing");
    assert_eq!(code, 3);
    assert!(io.err.contains("ifhf list"));
    assert_eq!(
        yes(&f, "remove org.freedesktop.Platform").0,
        3,
        "runtimes need --all"
    );
    assert_eq!(yes(&f, "remove org.freedesktop.Platform --all").0, 0);
}

#[test]
fn remove_partial_failure() {
    let mut f = Fixture::new();
    f.backend.fail_on = vec!["discord".into()];
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    f.backend
        .add_installed("com.discordapp.Discord", "Discord", "flathub", "user");
    let (code, io) = yes(&f, "remove spotify discord");
    assert_eq!(code, 1);
    assert!(io.out.contains("Removed: Spotify"));
    assert!(io.err.contains("Failed: Discord"), "{}", io.err);
    assert!(io.err.contains("Could not remove Discord."), "{}", io.err);
}

// ---- list / remotes / search / info ---------------------------------------------------

#[test]
fn list_hides_runtimes_and_filters() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    f.backend
        .add_installed("org.mozilla.firefox", "Firefox", "flathub", "user");
    f.backend
        .installed
        .lock()
        .unwrap()
        .push(ifhf::core::models::Installed {
            id: "org.freedesktop.Platform".into(),
            name: "Platform".into(),
            branch: "1".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            installation: "system".into(),
            is_runtime: true,
        });
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "list"), 0);
    assert_eq!(
        io.out,
        "Firefox\torg.mozilla.firefox\tstable\tuser\tflathub\nSpotify\tcom.spotify.Client\tstable\tsystem\tflathub\n"
    );
    let mut io2 = TestIo::non_interactive();
    assert_eq!(f.run(&mut io2, "-Q"), 0);
    assert_eq!(io.out, io2.out);
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "list --all");
    assert!(io.out.contains("org.freedesktop.Platform"));
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "list --search SPOT");
    assert_eq!(io.out.lines().count(), 1);
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "-Qs fire");
    assert!(io.out.contains("org.mozilla.firefox") && !io.out.contains("Spotify"));
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "list --user");
    assert_eq!(io.out.lines().count(), 1);
}

#[test]
fn list_on_a_terminal_has_a_header() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let mut io = TestIo::non_interactive();
    io.tty = true;
    f.run(&mut io, "list");
    assert!(io.out.starts_with("Name"), "{}", io.out);
    assert!(!io.out.contains('\x1b'), "no colour unless allowed");
}

#[test]
fn remotes_lists_scope_and_status() {
    let mut f = Fixture::new();
    f.backend.remotes.push(Remote {
        name: "old".into(),
        url: "u".into(),
        scope: Scope::User,
        enabled: false,
    });
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "remotes"), 0);
    assert!(io.out.contains("flathub\tsystem\tenabled\thttps://flathub"));
    assert!(io.out.contains("old\tuser\tdisabled\tu"));
}

#[test]
fn search_shows_name_id_remote_summary_ranked() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search browser"), 0, "{}", io.err);
    let ff = io.out.find("org.mozilla.firefox").expect(&io.out);
    let ch = io.out.find("org.chromium.Chromium").expect(&io.out);
    assert!(ff < ch, "keyword match ranks above summary match");
    assert!(io.out.contains(
        "Firefox\n  org.mozilla.firefox\n  Fast, private web browser\n  Remote: flathub\n"
    ));
    let mut io2 = TestIo::non_interactive();
    f.run(&mut io2, "-Ss browser");
    assert_eq!(io.out, io2.out);
}

#[test]
fn search_exact_id_and_name_rank_first() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search spotify");
    assert!(
        io.out.starts_with("Spotify\n  com.spotify.Client"),
        "{}",
        io.out
    );
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search org.mozilla.Firefox");
    assert!(io.out.contains("Remote: fedora"));
    assert!(io.out.find("Remote: fedora") < io.out.find("Remote: flathub"));
}

#[test]
fn search_empty_result_and_limit() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search zzqqnothing"), 3);
    assert!(
        io.err
            .contains("No applications found matching 'zzqqnothing'")
    );
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search web --limit 1"), 0);
    assert!(io.out.contains("Showing 1 of"), "{}", io.out);
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search o --limit 0");
    assert!(!io.out.contains("Showing"));
}

#[test]
fn search_honours_config_switches() {
    let mut f = Fixture::new();
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search annotations");
    assert!(io.out.contains("org.d.Docs"));
    f.config = Config::parse("[search]\ninclude_descriptions = false\n", "t").unwrap();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search annotations"), 3);
}

#[test]
fn search_remote_filter() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search firefox --remote fedora");
    assert!(io.out.contains("Remote: fedora") && !io.out.contains("Remote: flathub"));
}

#[test]
fn info_shows_details_and_install_status() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "info spotify"), 0, "{}", io.err);
    for want in [
        "Spotify",
        "Application:  com.spotify.Client",
        "Remote:       flathub",
        "Branch:       stable",
        "Architecture: x86_64",
        "Installed:    yes - system",
        "Stream music.",
    ] {
        assert!(io.out.contains(want), "missing {want:?} in\n{}", io.out);
    }
    assert!(
        io.out.contains("Homepage:     not available"),
        "missing metadata is stated, not invented"
    );
    let mut io2 = TestIo::non_interactive();
    f.run(&mut io2, "-Si spotify");
    assert_eq!(io.out, io2.out);
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "info discord");
    assert!(io.out.contains("Installed:    no"));
}

#[test]
fn info_for_ambiguous_and_unknown() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "info firefox"), 4);
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "info zzqqnothing"), 3);
}

#[test]
fn info_falls_back_to_installed_apps_missing_from_the_catalogue() {
    let f = Fixture::new();
    f.backend
        .add_installed("org.local.Only", "Local Only", "flathub", "system");
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "info org.local.Only"), 0, "{}", io.err);
    assert!(io.out.contains("Local Only") && io.out.contains("Installed:    yes"));
    assert!(io.out.contains("Description:  not available"));
}

// ---- updates / sync ------------------------------------------------------------------

fn upd(id: &str, name: &str, inst: &str) -> Update {
    Update {
        id: id.into(),
        name: name.into(),
        branch: "stable".into(),
        origin: "flathub".into(),
        installation: inst.into(),
        is_runtime: false,
    }
}

#[test]
fn updates_lists_or_says_up_to_date() {
    let mut f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "updates"), 0);
    assert!(io.out.contains("Everything is up to date."));
    f.backend.updates = vec![upd("com.spotify.Client", "Spotify", "system")];
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "updates");
    assert!(io.out.contains("com.spotify.Client"));
    let mut io2 = TestIo::non_interactive();
    f.run(&mut io2, "-Qu");
    assert_eq!(io.out, io2.out);
}

#[test]
fn update_all_confirms_then_updates_each_installation() {
    let mut f = Fixture::new();
    f.backend.updates = vec![
        upd("com.spotify.Client", "Spotify", "system"),
        upd("org.a.B", "B", "user"),
    ];
    let mut io = TestIo::answering(&["y"]);
    assert_eq!(f.run(&mut io, "update"), 0, "{}", io.err);
    assert_eq!(f.backend.calls(), vec!["update system ", "update user "]);
    assert_eq!(io.asked, vec!["Apply these updates? [Y/n] "]);
    let g = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(
        g.run(&mut io, "-Syu"),
        0,
        "nothing to do needs no confirmation"
    );
    assert!(g.backend.calls().is_empty());
}

#[test]
fn update_selected_application() {
    let mut f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    f.backend
        .add_installed("com.discordapp.Discord", "Discord", "flathub", "system");
    f.backend.updates = vec![
        upd("com.spotify.Client", "Spotify", "system"),
        upd("com.discordapp.Discord", "Discord", "system"),
    ];
    let (code, _) = yes(&f, "update spotify");
    assert_eq!(code, 0);
    assert_eq!(f.backend.calls(), vec!["update system com.spotify.Client"]);
    let (code, io) = yes(&f, "update zzqqnothing");
    assert_eq!(code, 3, "{}", io.err);
    f.backend.updates.clear();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "update spotify"), 0);
    assert!(io.out.contains("Already up to date: Spotify"));
}

#[test]
fn update_requires_yes_when_not_interactive() {
    let mut f = Fixture::new();
    f.backend.updates = vec![upd("com.spotify.Client", "Spotify", "system")];
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "update"), 5);
    assert!(f.backend.calls().is_empty());
}

#[test]
fn sync_refreshes_flatpak_then_rebuilds_the_index() {
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "sync"), 0, "{}", io.err);
    assert_eq!(f.backend.calls(), vec!["sync"]);
    assert!(io.out.contains("applications indexed"));
    assert!(f.cache.join("index.tsv").exists());
    let mut io2 = TestIo::non_interactive();
    f.run(&mut io2, "-Sy");
    assert_eq!(f.backend.calls(), vec!["sync", "sync"]);
}

// ---- metadata & cache ----------------------------------------------------------------

#[test]
fn second_search_uses_the_index_and_changes_invalidate_it() {
    let f = Fixture::new();
    let load = || ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache)).unwrap();
    assert!(!load().from_cache);
    assert!(load().from_cache);
    // Flatpak refreshes metadata: new content -> index is rebuilt.
    write_metadata(
        &f.system_dir(),
        "flathub",
        &flathub_xml().replace("Chromium", "Chromiumm"),
    );
    let c = load();
    assert!(!c.from_cache);
    assert!(c.apps.iter().any(|a| a.name == "Chromiumm"));
    assert!(load().from_cache);
}

#[test]
fn removing_a_remote_invalidates_the_index() {
    let f = Fixture::new();
    ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache)).unwrap();
    std::fs::remove_dir_all(f.system_dir().join("appstream").join("fedora")).unwrap();
    let mut f = f;
    f.backend.remotes.retain(|r| r.name != "fedora");
    let c = ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache)).unwrap();
    assert!(!c.from_cache);
    assert!(c.apps.iter().all(|a| a.remote != "fedora"));
}

#[test]
fn cache_can_be_disabled_and_deleted() {
    let mut f = Fixture::new();
    f.config = Config::parse("[cache]\nenabled = false\n", "t").unwrap();
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search spotify"), 0);
    assert!(!f.cache.exists(), "disabled cache writes nothing");
    let f = Fixture::new();
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search spotify");
    std::fs::remove_dir_all(&f.cache).unwrap();
    let mut io2 = TestIo::non_interactive();
    assert_eq!(
        f.run(&mut io2, "search spotify"),
        0,
        "rebuilds after deletion"
    );
    assert_eq!(io.out, io2.out);
}

#[test]
fn max_age_forces_a_rebuild() {
    let mut f = Fixture::new();
    f.config = Config::parse("[cache]\nmax_age_hours = 0\n", "t").unwrap();
    ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache)).unwrap();
    assert!(
        !ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache))
            .unwrap()
            .from_cache
    );
}

#[test]
fn failed_refresh_keeps_last_good_index_and_is_not_an_empty_catalogue() {
    let f = Fixture::new();
    ifhf::metadata::load(&f.backend, &f.config, Some(&f.cache)).unwrap();
    let before = std::fs::read(f.cache.join("index.tsv")).unwrap();
    // Corrupt both sources: the refresh must fail loudly.
    write_metadata(&f.system_dir(), "flathub", "<components><component><id>x");
    write_metadata(&f.system_dir(), "fedora", "not xml at all <<<");
    let mut io = TestIo::non_interactive();
    let code = f.run(&mut io, "search spotify");
    assert_eq!(code, 1);
    assert!(io.err.contains("could not be read"), "{}", io.err);
    assert!(
        !io.err.contains("No applications found"),
        "must not look like an empty catalogue"
    );
    assert_eq!(std::fs::read(f.cache.join("index.tsv")).unwrap(), before);
}

#[test]
fn one_bad_remote_does_not_hide_the_others() {
    let f = Fixture::new();
    write_metadata(&f.system_dir(), "fedora", "<components><component>");
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search spotify"), 0, "{}", io.err);
    assert!(
        io.err
            .contains("Could not read metadata for remote 'fedora'"),
        "{}",
        io.err
    );
    assert!(io.out.contains("com.spotify.Client"));
}

#[test]
fn no_metadata_at_all_points_to_sync() {
    let mut f = Fixture::new();
    f.backend.dirs = vec![(Scope::System, f.dir.path().join("empty"))];
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search spotify"), 1);
    assert!(io.err.contains("ifhf sync"), "{}", io.err);
}

#[test]
fn invalid_records_are_skipped_with_a_note() {
    let f = Fixture::new();
    let xml = flathub_xml().replace("</components>", r#"<component type="desktop-application"><id>bad id!</id><name>Bad</name></component></components>"#);
    write_metadata(&f.system_dir(), "flathub", &xml);
    let mut io = TestIo::non_interactive();
    assert_eq!(f.run(&mut io, "search spotify"), 0);
    assert!(
        io.err.contains("Skipped 1 invalid application record"),
        "{}",
        io.err
    );
}

#[test]
fn disabled_remotes_are_not_searched() {
    let mut f = Fixture::new();
    f.backend
        .remotes
        .iter_mut()
        .find(|r| r.name == "fedora")
        .unwrap()
        .enabled = false;
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search firefox");
    assert!(!io.out.contains("Remote: fedora"));
}

// ---- misc ----------------------------------------------------------------------------

#[test]
fn usage_errors_exit_2_and_point_to_help() {
    let f = Fixture::new();
    for a in [
        "install",
        "frobnicate",
        "--bogus",
        "-Sy spotify",
        "install x --user --system",
    ] {
        let mut io = TestIo::non_interactive();
        assert_eq!(f.run(&mut io, a), 2, "{a}");
        assert!(io.err.contains("ifhf --help"), "{a}: {}", io.err);
    }
}

#[test]
fn help_and_version() {
    let f = Fixture::new();
    for a in ["", "--help", "help", "-h"] {
        let mut io = TestIo::non_interactive();
        assert_eq!(f.run(&mut io, a), 0);
        assert!(io.out.contains("USAGE") && io.out.contains("EXIT STATUS"));
    }
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "--version");
    assert_eq!(io.out, format!("ifhf {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn queries_with_shell_metacharacters_are_just_text() {
    let f = Fixture::new();
    let args: Vec<String> = ["install", "spotify; touch /tmp/pwned $(id)"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut io = TestIo::non_interactive();
    let code = f.run_args(&mut io, &[args[0].clone(), args[1].clone(), "--yes".into()]);
    assert_eq!(code, 3);
    assert!(f.backend.calls().is_empty());
}

#[test]
fn already_installed_apps_are_reported_not_reinstalled() {
    let f = Fixture::new();
    f.backend
        .add_installed("com.spotify.Client", "Spotify", "flathub", "system");
    let (code, io) = yes(&f, "install spotify");
    assert_eq!(code, 0);
    assert!(io.out.contains("Already installed: Spotify"), "{}", io.out);
    assert!(f.backend.calls().is_empty());
    let (code, io) = yes(&f, "install spotify discord");
    assert_eq!(code, 0);
    assert!(io.out.contains("Already installed: Spotify"));
    assert_eq!(f.backend.calls().len(), 1);
    assert!(f.backend.calls()[0].contains("Discord") && !f.backend.calls()[0].contains("spotify"));
}

#[test]
fn search_collapses_the_same_remote_in_two_installations() {
    let mut f = Fixture::new();
    write_metadata(&f.dir.path().join("user"), "flathub", &flathub_xml());
    f.backend.remotes.push(Remote {
        name: "flathub".into(),
        url: "u".into(),
        scope: Scope::User,
        enabled: true,
    });
    let mut io = TestIo::non_interactive();
    f.run(&mut io, "search discord");
    assert_eq!(
        io.out.matches("com.discordapp.Discord").count(),
        1,
        "{}",
        io.out
    );
    assert!(
        io.out.contains("Remote: flathub (system, user)"),
        "{}",
        io.out
    );
}
