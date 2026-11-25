//! Pure formatting: structured data in, strings out. No printing, no I/O.

use crate::core::models::{App, Installed};
use crate::core::resolver::Candidate;

/// How output is dressed. `fancy` (layout, symbols) is for terminals only; `color` additionally
/// needs colour to be allowed (config, `--no-color`, `NO_COLOR`); `unicode` picks symbols over
/// ASCII. When all three are off the output is plain, stable text for scripts and pipes.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub color: bool,
    pub fancy: bool,
    pub unicode: bool,
}

impl Style {
    pub const PLAIN: Style = Style {
        color: false,
        fancy: false,
        unicode: false,
    };

    fn paint(self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn bold(self, s: &str) -> String {
        self.paint("1", s)
    }
    pub fn dim(self, s: &str) -> String {
        self.paint("2", s)
    }
    pub fn red(self, s: &str) -> String {
        self.paint("31", s)
    }
    pub fn green(self, s: &str) -> String {
        self.paint("32", s)
    }
    pub fn yellow(self, s: &str) -> String {
        self.paint("33", s)
    }
    /// Application IDs and the accent colour: bright blue reads on light and dark themes.
    pub fn id(self, s: &str) -> String {
        self.paint("94", s)
    }
    pub fn bold_red(self, s: &str) -> String {
        self.paint("1;31", s)
    }

    fn sym(self, uni: &'static str, ascii: &'static str) -> &'static str {
        if self.unicode { uni } else { ascii }
    }
    pub fn ok_mark(self) -> String {
        self.green(self.sym("✓", "+"))
    }
    pub fn fail_mark(self) -> String {
        self.red(self.sym("✗", "x"))
    }
    pub fn warn_mark(self) -> String {
        self.yellow("!")
    }
    pub fn step_mark(self) -> String {
        self.id(self.sym("→", "->"))
    }
    fn bar(self) -> String {
        self.id(self.sym("┃", "|"))
    }
    fn dot(self) -> &'static str {
        self.sym("·", "-")
    }
}

/// Aligned table with a header on a terminal; plain tab-separated rows (no header) otherwise.
/// `paint` colours a cell by column; padding is computed on the unpainted text.
pub fn table(
    header: &[&str],
    rows: &[Vec<String>],
    pretty: bool,
    style: Style,
    paint: &dyn Fn(usize, &str) -> String,
) -> String {
    let mut out = String::new();
    if !pretty {
        for r in rows {
            out.push_str(&r.join("\t"));
            out.push('\n');
        }
        return out;
    }
    let mut w: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            w[i] = w[i].max(c.chars().count());
        }
    }
    let line = |cells: Vec<&str>, head: bool| {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate() {
            let pad = " ".repeat(w[i] - c.chars().count());
            let cell = if head { style.dim(c) } else { paint(i, c) };
            s.push_str(&cell);
            if i + 1 < cells.len() {
                s.push_str(&pad);
                s.push_str("  ");
            }
        }
        s.push('\n');
        s
    };
    out.push_str(&line(header.to_vec(), true));
    for r in rows {
        out.push_str(&line(r.iter().map(String::as_str).collect(), false));
    }
    out
}

/// Greedy word wrap with a fixed indent.
pub fn wrap(text: &str, width: usize, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width - indent {
            out.push_str(&pad);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(&pad);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn missing(v: &str) -> &str {
    if v.is_empty() { "not available" } else { v }
}

pub fn search_results(apps: &[App], cands: &[Candidate], style: Style) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < cands.len() {
        let a = &apps[cands[i].index];
        // Same app from the same remote configured in several installations: show it once.
        let mut scopes = vec![a.scope.as_str()];
        while let Some(n) = cands.get(i + 1).map(|c| &apps[c.index]) {
            if n.id != a.id || n.remote != a.remote {
                break;
            }
            if !scopes.contains(&n.scope.as_str()) {
                scopes.push(n.scope.as_str());
            }
            i += 1;
        }
        let remote = if scopes.len() > 1 {
            format!("{} ({})", a.remote, scopes.join(", "))
        } else {
            a.remote.clone()
        };
        if style.fancy {
            out.push_str(&format!(
                "{}  {}\n",
                style.bold(missing(&a.name)),
                style.id(&a.id)
            ));
            if !a.summary.is_empty() {
                out.push_str(&format!("  {}\n", a.summary));
            }
            out.push_str(&format!("  {}\n\n", style.dim(&remote)));
        } else {
            out.push_str(&format!(
                "{}\n  {}\n",
                style.bold(missing(&a.name)),
                style.dim(&a.id)
            ));
            if !a.summary.is_empty() {
                out.push_str(&format!("  {}\n", a.summary));
            }
            out.push_str(&format!("  Remote: {remote}\n\n"));
        }
        i += 1;
    }
    out
}

/// Numbered list used when the user must choose between similar matches.
pub fn candidate_list(apps: &[App], cands: &[Candidate], style: Style) -> String {
    let mut out = String::new();
    for (n, c) in cands.iter().enumerate() {
        let a = &apps[c.index];
        if style.fancy {
            out.push_str(&format!(
                "{}  {}  {}\n     {}\n\n",
                // pad before colouring: escape codes would otherwise count towards the width
                style.bold(&style.id(&format!("{:>3}", n + 1))),
                style.bold(missing(&a.name)),
                style.id(&a.id),
                style.dim(&format!("{} {} {}", a.remote, style.dot(), a.scope)),
            ));
        } else {
            out.push_str(&format!(
                "{}. {}\n   {}\n   Remote: {} ({})\n\n",
                n + 1,
                style.bold(missing(&a.name)),
                a.id,
                a.remote,
                a.scope
            ));
        }
    }
    out
}

pub struct PlanLine<'a> {
    pub app: &'a App,
    /// Why the scope differs from the one the user asked for, if it does.
    pub scope_note: Option<String>,
}

pub fn install_plan(items: &[PlanLine], style: Style) -> String {
    let mut out = String::new();
    for it in items {
        let a = it.app;
        if style.fancy {
            let bar = style.bar();
            out.push_str(&format!("{bar} {}\n", style.bold(missing(&a.name))));
            if !a.summary.is_empty() {
                out.push_str(&format!("{bar} {}\n", a.summary));
            }
            out.push_str(&format!(
                "{bar}\n{bar} {} {}\n",
                style.dim("Application"),
                style.id(&a.id)
            ));
            out.push_str(&format!(
                "{bar} {} {}\n",
                style.dim("Remote     "),
                a.remote
            ));
            let scope = match &it.scope_note {
                Some(n) => format!("{} {}", a.scope, style.yellow(&format!("({n})"))),
                None => a.scope.to_string(),
            };
            out.push_str(&format!("{bar} {} {scope}\n\n", style.dim("Scope      ")));
            continue;
        }
        out.push_str(&format!("{}\n", style.bold(missing(&a.name))));
        if !a.summary.is_empty() {
            out.push_str(&format!("{}\n", a.summary));
        }
        out.push_str(&format!(
            "\nApplication: {}\nRemote:      {}\n",
            a.id, a.remote
        ));
        match &it.scope_note {
            Some(n) => out.push_str(&format!("Scope:       {} ({n})\n\n", a.scope)),
            None => out.push_str(&format!("Scope:       {}\n\n", a.scope)),
        }
    }
    out
}

pub fn remove_plan(items: &[Installed], delete_data: bool, style: Style) -> String {
    if style.fancy {
        let mut o = String::new();
        let bar = style.bar();
        for i in items {
            o.push_str(&format!(
                "{bar} {}  {}  {}\n",
                style.bold(&i.name),
                style.id(&i.id),
                style.dim(&i.installation)
            ));
        }
        o.push('\n');
        if delete_data {
            o.push_str(&format!(
                "{} {}\n\n",
                style.warn_mark(),
                style.bold_red("--delete-data permanently deletes these applications' user data (settings, saved files).")
            ));
        } else {
            o.push_str(&format!(
                "{} {}\n\n",
                style.ok_mark(),
                style.dim("Application data is kept; reinstalling restores it.")
            ));
        }
        return o;
    }
    let mut plan = String::from("The following will be removed:\n\n");
    for i in items {
        plan.push_str(&format!(
            "  {} ({}) [{}]\n",
            style.bold(&i.name),
            i.id,
            i.installation
        ));
    }
    plan.push('\n');
    if delete_data {
        plan.push_str(&style.red("WARNING: --delete-data permanently deletes these applications' user data (settings, saved files).\n\n"));
    } else {
        plan.push_str("Application data is kept; reinstalling restores it.\n\n");
    }
    plan
}

pub fn info(app: &App, installed: &[&Installed], style: Style) -> String {
    let mut o = format!("{}\n", style.bold(missing(&app.name)));
    if !app.summary.is_empty() {
        o.push_str(&format!("{}\n", app.summary));
    }
    let opt = |v: &Option<String>| v.as_deref().unwrap_or("not available").to_string();
    if style.fancy {
        let row = |k: &str, v: String| format!("{} {v}\n", style.dim(&format!("{k:<13}")));
        o.push('\n');
        o.push_str(&row("Application", style.id(&app.id)));
        o.push_str(&row("Remote", missing(&app.remote).to_string()));
        o.push_str(&row("Branch", opt(&app.branch)));
        o.push_str(&row("Architecture", opt(&app.arch)));
        o.push_str(&row("Developer", opt(&app.developer)));
        o.push_str(&row("Homepage", opt(&app.homepage)));
        if installed.is_empty() {
            o.push_str(&row("Installed", style.dim("no")));
        } else {
            let places: Vec<String> = installed
                .iter()
                .map(|i| format!("{} ({}, from {})", i.installation, i.branch, i.origin))
                .collect();
            o.push_str(&row(
                "Installed",
                format!("{} {}", style.ok_mark(), places.join("; ")),
            ));
        }
        o.push('\n');
        if app.description.is_empty() {
            o.push_str(&row("Description", style.dim("not available")));
        } else {
            o.push_str(&format!("{}\n", style.dim("Description")));
            o.push_str(&wrap(&app.description, 78, 2));
        }
        return o;
    }
    o.push_str(&format!(
        "\nApplication:  {}\nRemote:       {}\nBranch:       {}\nArchitecture: {}\nDeveloper:    {}\nHomepage:     {}\n",
        app.id,
        missing(&app.remote),
        opt(&app.branch),
        opt(&app.arch),
        opt(&app.developer),
        opt(&app.homepage),
    ));
    if installed.is_empty() {
        o.push_str("Installed:    no\n");
    } else {
        let places: Vec<String> = installed
            .iter()
            .map(|i| format!("{} ({}, from {})", i.installation, i.branch, i.origin))
            .collect();
        o.push_str(&format!("Installed:    yes - {}\n", places.join("; ")));
    }
    if app.description.is_empty() {
        o.push_str("\nDescription:  not available\n");
    } else {
        o.push_str("\nDescription:\n");
        o.push_str(&wrap(&app.description, 78, 2));
    }
    o
}

/// "Installed: A, B" in plain mode; one checked line per application on a terminal.
pub fn done_lines(style: Style, verb: &str, names: &[String]) -> String {
    if !style.fancy {
        return format!("{verb}: {}\n", names.join(", "));
    }
    names
        .iter()
        .map(|n| format!("{} {verb} {}\n", style.ok_mark(), style.bold(n)))
        .collect()
}

/// A warning shown above the real output ("Note: ..." in plain mode).
pub fn note(style: Style, text: &str) -> String {
    if style.fancy {
        format!("{} {}\n", style.warn_mark(), style.yellow(text))
    } else {
        format!("Note: {text}\n")
    }
}

/// An error for stderr. Plain: `ifhf: message`. Terminal: a red mark and a bold first line.
pub fn error(style: Style, msg: &str, tail: &str) -> String {
    if !style.fancy {
        return format!("ifhf: {msg}{tail}\n");
    }
    let mut lines = msg.lines();
    let first = lines.next().unwrap_or("");
    let mut out = format!("{} {}\n", style.fail_mark(), style.bold_red(first));
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    if !tail.is_empty() {
        out.push_str(&style.dim(tail.trim_start_matches('\n')));
        out.push('\n');
    }
    out
}

/// The user said no. Not a failure, so no alarm colours on a terminal.
pub fn cancelled(style: Style) -> String {
    if style.fancy {
        format!("{}\n", style.dim("Cancelled."))
    } else {
        "ifhf: Cancelled.\n".to_string()
    }
}

/// `[Y/n]` with the default highlighted on a terminal.
pub fn prompt(style: Style, question: &str, default_yes: bool) -> String {
    if !style.fancy {
        let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
        return format!("{question} {hint} ");
    }
    let hint = if default_yes {
        format!("[{}/n]", style.bold("Y"))
    } else {
        format!("[y/{}]", style.bold("N"))
    };
    format!("{} {hint} ", style.bold(question))
}

/// The help text, with headings bold and command/option names accented on a terminal.
pub fn help(text: &str, style: Style) -> String {
    if !style.fancy {
        return text.to_string();
    }
    let mut out = String::new();
    let mut section = "";
    for line in text.lines() {
        if let Some(first) = line.chars().next().filter(|c| c.is_ascii_uppercase()) {
            let _ = first;
            let head = line.split_whitespace().next().unwrap_or("");
            if head.chars().all(|c| c.is_ascii_uppercase()) && head.len() > 2 {
                section = if head == "EXIT" { "exit" } else { "" };
                out.push_str(&style.bold(line));
                out.push('\n');
                continue;
            }
        }
        if section != "exit" && line.starts_with("  ") && !line.starts_with("   ") {
            let rest = &line[2..];
            if let Some(i) = rest.find("  ") {
                out.push_str(&format!("  {}{}\n", style.id(&rest[..i]), &rest[i..]));
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style::PLAIN;

    #[test]
    fn table_modes() {
        let rows = vec![
            vec!["a".to_string(), "bbb".to_string()],
            vec!["cc".to_string(), "d".to_string()],
        ];
        assert_eq!(
            table(&["X", "Y"], &rows, false, PLAIN, &|_, c| c.to_string()),
            "a\tbbb\ncc\td\n"
        );
        let t = table(&["X", "Y"], &rows, true, PLAIN, &|_, c| c.to_string());
        assert_eq!(t, "X   Y\na   bbb\ncc  d\n");
    }

    #[test]
    fn color_is_never_the_only_signal() {
        let a = App::new("org.a.B", "Bee", "r");
        let plain = search_results(
            std::slice::from_ref(&a),
            &[Candidate {
                index: 0,
                score: 1,
                kind: crate::core::resolver::MatchKind::ExactName,
            }],
            PLAIN,
        );
        assert!(!plain.contains('\x1b'));
        assert!(plain.contains("Bee") && plain.contains("org.a.B") && plain.contains("Remote: r"));
    }

    #[test]
    fn info_marks_missing_metadata() {
        let a = App::new("org.a.B", "Bee", "r");
        let s = info(&a, &[], PLAIN);
        assert!(s.contains("Homepage:     not available"));
        assert!(s.contains("Description:  not available"));
        assert!(s.contains("Installed:    no"));
    }

    #[test]
    fn wraps_long_text() {
        let w = wrap(&"word ".repeat(40), 30, 2);
        assert!(
            w.lines()
                .all(|l| l.chars().count() <= 30 && l.starts_with("  "))
        );
        assert!(w.lines().count() > 3);
    }

    const COLOR: Style = Style {
        color: true,
        fancy: true,
        unicode: true,
    };
    const NOCOLOR: Style = Style {
        color: false,
        fancy: true,
        unicode: true,
    };
    const ASCII: Style = Style {
        color: false,
        fancy: true,
        unicode: false,
    };

    fn strip(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c == '\x1b' {
                for d in it.by_ref() {
                    if d == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn app() -> App {
        let mut a = App::new("org.a.Bee", "Bee", "flathub");
        a.summary = "Makes honey".into();
        a.description = "A long description of bees.".into();
        a
    }

    fn inst() -> Installed {
        Installed {
            id: "org.a.Bee".into(),
            name: "Bee".into(),
            branch: "stable".into(),
            arch: "x86_64".into(),
            origin: "flathub".into(),
            installation: "system".into(),
            is_runtime: false,
        }
    }

    fn all_views(st: Style) -> Vec<String> {
        let a = app();
        let apps = vec![a.clone()];
        let cands = vec![Candidate {
            index: 0,
            score: 1,
            kind: crate::core::resolver::MatchKind::ExactName,
        }];
        let i = inst();
        vec![
            search_results(&apps, &cands, st),
            candidate_list(&apps, &cands, st),
            install_plan(
                &[PlanLine {
                    app: &a,
                    scope_note: Some("why".into()),
                }],
                st,
            ),
            remove_plan(std::slice::from_ref(&i), false, st),
            remove_plan(std::slice::from_ref(&i), true, st),
            info(&a, &[&i], st),
            info(&a, &[], st),
            done_lines(st, "Installed", &["Bee".into(), "Ant".into()]),
            note(st, "careful"),
            error(st, "bad thing\nsecond line", "\n\nTry: ifhf --help"),
            prompt(st, "Install Bee?", true),
        ]
    }

    #[test]
    fn colour_never_changes_the_text() {
        for (c, n) in all_views(COLOR).iter().zip(all_views(NOCOLOR)) {
            assert!(
                c.contains("\x1b["),
                "colour mode must colour something: {c:?}"
            );
            assert_eq!(strip(c), n, "colour only adds escape codes");
            assert!(!n.contains('\x1b'));
        }
    }

    #[test]
    fn ascii_fallback_is_really_ascii() {
        for v in all_views(ASCII) {
            assert!(v.is_ascii(), "non-ASCII in fallback output: {v:?}");
        }
        assert!(
            all_views(NOCOLOR).iter().any(|v| !v.is_ascii()),
            "unicode mode uses symbols"
        );
    }

    #[test]
    fn plain_mode_has_no_escapes_or_glyphs() {
        for v in all_views(PLAIN) {
            assert!(v.is_ascii() && !v.contains('\x1b'), "{v:?}");
        }
        assert_eq!(all_views(PLAIN)[10], "Install Bee? [Y/n] ");
        assert_eq!(
            done_lines(PLAIN, "Installed", &["A".into(), "B".into()]),
            "Installed: A, B\n"
        );
        assert_eq!(note(PLAIN, "x"), "Note: x\n");
        assert_eq!(error(PLAIN, "boom", ""), "ifhf: boom\n");
    }

    #[test]
    fn terminal_layout_uses_the_accent_bar_and_checks() {
        let v = all_views(NOCOLOR);
        assert!(
            v[2].contains("┃ Bee") && v[2].contains("┃ Application org.a.Bee"),
            "{}",
            v[2]
        );
        assert!(v[2].contains("Scope"), "{}", v[2]);
        assert!(
            v[7].starts_with("✓ Installed Bee\n✓ Installed Ant\n"),
            "{}",
            v[7]
        );
        assert!(v[9].starts_with("✗ bad thing\nsecond line\n"), "{}", v[9]);
        assert!(
            v[5].contains("Installed     ✓ system (stable, from flathub)"),
            "{}",
            v[5]
        );
        assert!(
            v[0].starts_with("Bee  org.a.Bee\n  Makes honey\n  flathub\n"),
            "{}",
            v[0]
        );
        assert!(
            v[4].contains("--delete-data permanently deletes"),
            "{}",
            v[4]
        );
        assert!(v[3].contains("Application data is kept"), "{}", v[3]);
    }

    #[test]
    fn prompt_highlights_the_default_answer() {
        assert!(prompt(COLOR, "Q?", true).contains("\x1b[1mY\x1b[0m/n"));
        assert!(prompt(COLOR, "Q?", false).contains("y/\x1b[1mN\x1b[0m"));
        assert_eq!(strip(&prompt(COLOR, "Q?", false)), "Q? [y/N] ");
    }

    #[test]
    fn help_is_styled_without_changing_its_words() {
        let styled = help(crate::cli::args::HELP, COLOR);
        assert!(styled.contains("\x1b[1mUSAGE\x1b[0m") && styled.contains("\x1b[94m"));
        assert_eq!(strip(&styled), crate::cli::args::HELP);
        assert_eq!(help(crate::cli::args::HELP, PLAIN), crate::cli::args::HELP);
    }

    #[test]
    fn coloured_tables_keep_their_alignment() {
        let rows = vec![
            vec!["a".to_string(), "bbb".to_string()],
            vec!["cc".to_string(), "d".to_string()],
        ];
        let plain = table(&["X", "Y"], &rows, true, NOCOLOR, &|_, c| c.to_string());
        let colour = table(&["X", "Y"], &rows, true, COLOR, &|i, c| {
            if i == 0 { COLOR.bold(c) } else { COLOR.id(c) }
        });
        assert_eq!(strip(&colour), plain);
        assert!(colour.contains("\x1b[94mbbb"));
    }
}
