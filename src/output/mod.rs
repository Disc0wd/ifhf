//! Pure formatting: structured data in, strings out. No printing, no I/O.

use crate::core::models::{App, Installed};
use crate::core::resolver::Candidate;

#[derive(Clone, Copy)]
pub struct Style {
    pub color: bool,
}

impl Style {
    pub fn bold(self, s: &str) -> String {
        if self.color {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn dim(self, s: &str) -> String {
        if self.color {
            format!("\x1b[2m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn red(self, s: &str) -> String {
        if self.color {
            format!("\x1b[31m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

/// Aligned table with a header on a terminal; plain tab-separated rows (no header) otherwise.
pub fn table(header: &[&str], rows: &[Vec<String>], pretty: bool, style: Style) -> String {
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
    let line = |cells: Vec<&str>, bold: bool| {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate() {
            let pad = " ".repeat(w[i] - c.chars().count());
            let cell = if bold { style.bold(c) } else { c.to_string() };
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
        out.push_str(&format!(
            "{}\n  {}\n",
            style.bold(missing(&a.name)),
            style.dim(&a.id)
        ));
        if !a.summary.is_empty() {
            out.push_str(&format!("  {}\n", a.summary));
        }
        if scopes.len() > 1 {
            out.push_str(&format!(
                "  Remote: {} ({})\n\n",
                a.remote,
                scopes.join(", ")
            ));
        } else {
            out.push_str(&format!("  Remote: {}\n\n", a.remote));
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
        out.push_str(&format!(
            "{}. {}\n   {}\n   Remote: {} ({})\n\n",
            n + 1,
            style.bold(missing(&a.name)),
            a.id,
            a.remote,
            a.scope
        ));
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

pub fn info(app: &App, installed: &[&Installed], style: Style) -> String {
    let mut o = format!("{}\n", style.bold(missing(&app.name)));
    if !app.summary.is_empty() {
        o.push_str(&format!("{}\n", app.summary));
    }
    let opt = |v: &Option<String>| v.as_deref().unwrap_or("not available").to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style { color: false };

    #[test]
    fn table_modes() {
        let rows = vec![
            vec!["a".to_string(), "bbb".to_string()],
            vec!["cc".to_string(), "d".to_string()],
        ];
        assert_eq!(table(&["X", "Y"], &rows, false, PLAIN), "a\tbbb\ncc\td\n");
        let t = table(&["X", "Y"], &rows, true, PLAIN);
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
}
