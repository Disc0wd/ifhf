//! Deterministic name -> application resolution. Pure functions over `App` slices;
//! no I/O and no formatting, so it is testable on its own.

use super::models::{App, Scope};

/// Two different candidates closer than this many points are "similarly relevant".
pub const AMBIGUITY_MARGIN: u32 = 100;

/// Smaller catalogues are scanned on one thread; spawning costs more than it saves.
const PARALLEL_MIN_APPS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    Description,
    Summary,
    Keyword,
    IdPartial,
    NamePartial,
    NameTokens,
    NamePrefix,
    NormalizedName,
    ExactName,
    ExactId,
}

impl MatchKind {
    pub fn score(self) -> u32 {
        match self {
            MatchKind::ExactId => 1000,
            MatchKind::ExactName => 900,
            MatchKind::NormalizedName => 850,
            MatchKind::NamePrefix => 700,
            MatchKind::NameTokens => 600,
            MatchKind::NamePartial => 450,
            MatchKind::IdPartial => 400,
            MatchKind::Keyword => 300,
            MatchKind::Summary => 200,
            MatchKind::Description => 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// Index into the slice that was searched.
    pub index: usize,
    pub score: u32,
    pub kind: MatchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// Exactly one candidate, matched by exact application ID.
    Exact,
    /// One clearly best candidate.
    Unique,
    /// Several similarly relevant candidates; the caller must ask.
    Ambiguous,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct ResolutionResult {
    pub query: String,
    pub candidates: Vec<Candidate>,
    pub status: ResolutionStatus,
}

#[derive(Debug, Clone, Default)]
pub struct SearchOpts {
    pub fuzzy: bool,
    pub descriptions: bool,
    pub remote: Option<String>,
    pub scope: Option<Scope>,
}

/// Trim, case-fold and collapse whitespace. IDs are compared before this is applied.
pub fn normalize(q: &str) -> String {
    q.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Lowercase, punctuation to spaces, whitespace collapsed ("GNOME-Console" == "gnome console").
fn fold(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>();
    normalize(&mapped)
}

fn contains_all(hay: &str, tokens: &[&str]) -> bool {
    tokens.iter().all(|t| hay.contains(t))
}

fn classify(app: &App, raw: &str, q: &str, qfold: &str, opts: &SearchOpts) -> Option<MatchKind> {
    if app.id == raw {
        return Some(MatchKind::ExactId);
    }
    let name = app.name.to_lowercase();
    if name == q {
        return Some(MatchKind::ExactName);
    }
    let nfold = fold(&name);
    if !qfold.is_empty() && nfold == qfold {
        return Some(MatchKind::NormalizedName);
    }
    if name.starts_with(q) || (!qfold.is_empty() && nfold.starts_with(qfold)) {
        return Some(MatchKind::NamePrefix);
    }
    let tokens: Vec<&str> = q.split(' ').collect();
    let ftokens: Vec<&str> = qfold.split(' ').filter(|t| !t.is_empty()).collect();
    if !ftokens.is_empty() {
        let words: Vec<&str> = nfold.split(' ').collect();
        if ftokens.iter().all(|t| words.contains(t)) {
            return Some(MatchKind::NameTokens);
        }
    }
    if opts.fuzzy {
        if name.contains(q) || contains_all(&name, &tokens) {
            return Some(MatchKind::NamePartial);
        }
        if app.id.to_lowercase().contains(q) {
            return Some(MatchKind::IdPartial);
        }
    }
    if app.keywords.iter().any(|k| {
        let k = k.to_lowercase();
        tokens.iter().all(|t| k.contains(t))
    }) || (tokens.len() > 1 && {
        let all = app.keywords.join(" ").to_lowercase();
        contains_all(&all, &tokens)
    }) {
        return Some(MatchKind::Keyword);
    }
    if !app.summary.is_empty() && contains_all(&app.summary.to_lowercase(), &tokens) {
        return Some(MatchKind::Summary);
    }
    if opts.descriptions
        && !app.description.is_empty()
        && contains_all(&app.description.to_lowercase(), &tokens)
    {
        return Some(MatchKind::Description);
    }
    None
}

/// All matching candidates, best first. Ties break on name, ID, remote: fully deterministic.
pub fn search(apps: &[App], query: &str, opts: &SearchOpts) -> Vec<Candidate> {
    let raw = query.trim();
    let q = normalize(raw);
    if q.is_empty() {
        return Vec::new();
    }
    let qfold = fold(&q);
    let scan = |base: usize, slice: &[App]| -> Vec<Candidate> {
        slice
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                opts.remote.as_deref().is_none_or(|r| a.remote == r)
                    && opts.scope.is_none_or(|s| a.scope == s)
            })
            .filter_map(|(i, a)| {
                classify(a, raw, &q, &qfold, opts).map(|kind| Candidate {
                    index: base + i,
                    score: kind.score(),
                    kind,
                })
            })
            .collect()
    };
    // The scan is independent per app: split it across cores for large catalogues.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8);
    let mut out: Vec<Candidate> = if workers < 2 || apps.len() < PARALLEL_MIN_APPS {
        scan(0, apps)
    } else {
        let size = apps.len().div_ceil(workers);
        std::thread::scope(|s| {
            let handles: Vec<_> = apps
                .chunks(size)
                .enumerate()
                .map(|(n, c)| {
                    let scan = &scan;
                    s.spawn(move || scan(n * size, c))
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("search worker panicked"))
                .collect()
        })
    };
    out.sort_by(|x, y| {
        let (a, b) = (&apps[x.index], &apps[y.index]);
        y.score
            .cmp(&x.score)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.remote.cmp(&b.remote))
            .then_with(|| a.scope.as_str().cmp(b.scope.as_str()))
            .then_with(|| a.branch.cmp(&b.branch))
    });
    out
}

pub fn resolve(apps: &[App], query: &str, opts: &SearchOpts) -> ResolutionResult {
    let candidates = search(apps, query, opts);
    let status = match candidates.first() {
        None => ResolutionStatus::NotFound,
        Some(top) => {
            let t = &apps[top.index];
            let rival = candidates[1..].iter().find(|c| {
                apps[c.index].identity() != t.identity() && c.score + AMBIGUITY_MARGIN > top.score
            });
            match (rival, top.kind) {
                (Some(_), _) => ResolutionStatus::Ambiguous,
                (None, MatchKind::ExactId) => ResolutionStatus::Exact,
                (None, _) => ResolutionStatus::Unique,
            }
        }
    };
    ResolutionResult {
        query: query.trim().to_string(),
        candidates,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<App> {
        let mut firefox = App::new("org.mozilla.firefox", "Firefox", "flathub");
        firefox.summary = "Fast, private and open-source web browser".into();
        firefox.keywords = vec!["Browser".into(), "Internet".into()];
        let mut chromium = App::new("org.chromium.Chromium", "Chromium", "flathub");
        chromium.summary = "Open-source web browser".into();
        let mut spotify = App::new("com.spotify.Client", "Spotify", "flathub");
        spotify.summary = "Music streaming".into();
        let mut tui = App::new("org.x.spotify_tui", "Spotify TUI", "flathub");
        tui.summary = "Terminal client".into();
        let mut console = App::new("org.gnome.Console", "GNOME Console", "flathub");
        console.summary = "A simple terminal".into();
        let mut ptyxis = App::new("org.gnome.Ptyxis", "Ptyxis", "flathub");
        ptyxis.summary = "Container-oriented terminal".into();
        let mut desc = App::new("org.d.Doc", "Docs", "flathub");
        desc.description = "Edits PDF files with annotations".into();
        let fedora_ff = App::new("org.mozilla.Firefox", "Firefox", "fedora");
        vec![
            firefox, chromium, spotify, tui, console, ptyxis, desc, fedora_ff,
        ]
    }

    fn opts() -> SearchOpts {
        SearchOpts {
            fuzzy: true,
            descriptions: true,
            ..SearchOpts::default()
        }
    }

    #[test]
    fn normalisation() {
        assert_eq!(normalize("  Visual   STUDIO\tCode "), "visual studio code");
        assert_eq!(fold("GNOME-Console"), "gnome console");
    }

    #[test]
    fn exact_id_beats_everything() {
        let apps = catalog();
        let r = resolve(&apps, "org.mozilla.firefox", &opts());
        assert_eq!(r.status, ResolutionStatus::Exact);
        assert_eq!(apps[r.candidates[0].index].remote, "flathub");
        assert_eq!(r.candidates[0].kind, MatchKind::ExactId);
    }

    #[test]
    fn ids_are_case_sensitive_exact() {
        let apps = catalog();
        let r = resolve(&apps, "org.mozilla.Firefox", &opts());
        assert_eq!(r.status, ResolutionStatus::Exact);
        assert_eq!(apps[r.candidates[0].index].remote, "fedora");
    }

    #[test]
    fn unique_exact_name() {
        let apps = catalog();
        let r = resolve(&apps, "spotify", &opts());
        assert_eq!(r.status, ResolutionStatus::Unique);
        assert_eq!(apps[r.candidates[0].index].id, "com.spotify.Client");
        assert_eq!(r.candidates[0].kind, MatchKind::ExactName);
        assert_eq!(apps[r.candidates[1].index].id, "org.x.spotify_tui");
    }

    #[test]
    fn same_name_two_remotes_is_ambiguous() {
        let apps = catalog();
        let r = resolve(&apps, "firefox", &opts());
        assert_eq!(r.status, ResolutionStatus::Ambiguous);
        assert_eq!(r.candidates.len(), 2);
    }

    #[test]
    fn remote_filter_disambiguates() {
        let apps = catalog();
        let o = SearchOpts {
            remote: Some("fedora".into()),
            ..opts()
        };
        let r = resolve(&apps, "firefox", &o);
        assert_eq!(r.status, ResolutionStatus::Unique);
        assert_eq!(apps[r.candidates[0].index].remote, "fedora");
    }

    #[test]
    fn terminal_is_ambiguous_never_silently_chosen() {
        let apps = catalog();
        let r = resolve(&apps, "terminal", &opts());
        assert_eq!(r.status, ResolutionStatus::Ambiguous);
    }

    #[test]
    fn ranking_order_and_kinds() {
        let apps = catalog();
        let r = search(&apps, "browser", &opts());
        let kinds: Vec<_> = r
            .iter()
            .map(|c| (apps[c.index].id.as_str(), c.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("org.mozilla.firefox", MatchKind::Keyword),
                ("org.chromium.Chromium", MatchKind::Summary),
            ]
        );
    }

    #[test]
    fn prefix_and_partial_and_punctuation() {
        let apps = catalog();
        assert_eq!(
            search(&apps, "gnome-console", &opts())[0].kind,
            MatchKind::NormalizedName
        );
        assert_eq!(
            search(&apps, "fire", &opts())[0].kind,
            MatchKind::NamePrefix
        );
        assert_eq!(
            search(&apps, "console", &opts())[0].kind,
            MatchKind::NameTokens
        );
        assert_eq!(
            search(&apps, "romiu", &opts())[0].kind,
            MatchKind::NamePartial
        );
        assert_eq!(
            search(&apps, "chromium.chro", &opts())[0].kind,
            MatchKind::IdPartial
        );
    }

    #[test]
    fn config_switches() {
        let apps = catalog();
        let strict = SearchOpts {
            fuzzy: false,
            descriptions: false,
            ..SearchOpts::default()
        };
        assert!(search(&apps, "romiu", &strict).is_empty());
        assert!(search(&apps, "annotations", &strict).is_empty());
        assert_eq!(
            search(&apps, "annotations", &opts())[0].kind,
            MatchKind::Description
        );
    }

    #[test]
    fn not_found_and_empty_query() {
        let apps = catalog();
        assert_eq!(
            resolve(&apps, "zzzzqq", &opts()).status,
            ResolutionStatus::NotFound
        );
        assert_eq!(
            resolve(&apps, "   ", &opts()).status,
            ResolutionStatus::NotFound
        );
    }

    #[test]
    fn missing_metadata_fields_do_not_break_matching() {
        let apps = vec![App::new("org.a.Bare", "", "r")];
        assert_eq!(
            resolve(&apps, "org.a.Bare", &opts()).status,
            ResolutionStatus::Exact
        );
        // No name to match, but the ID still can (fuzzy), and keywords/summary are simply absent.
        assert_eq!(
            resolve(&apps, "bare", &opts()).candidates[0].kind,
            MatchKind::IdPartial
        );
        assert_eq!(
            resolve(&apps, "nothing", &opts()).status,
            ResolutionStatus::NotFound
        );
    }

    #[test]
    fn scope_filter_and_duplicates_across_scopes() {
        let mut a = App::new("org.a.B", "Bee", "flathub");
        a.scope = Scope::User;
        let mut b = App::new("org.a.B", "Bee", "flathub");
        b.scope = Scope::System;
        let apps = vec![a, b];
        assert_eq!(
            resolve(&apps, "bee", &opts()).status,
            ResolutionStatus::Ambiguous
        );
        let o = SearchOpts {
            scope: Some(Scope::System),
            ..opts()
        };
        assert_eq!(resolve(&apps, "bee", &o).status, ResolutionStatus::Unique);
    }

    #[test]
    fn deterministic() {
        let apps = catalog();
        let a = search(&apps, "o", &opts());
        let b = search(&apps, "o", &opts());
        assert_eq!(a, b);
    }

    #[test]
    fn parallel_scan_matches_sequential_scan() {
        let apps: Vec<App> = (0..(PARALLEL_MIN_APPS * 3 + 7))
            .map(|i| {
                let mut a = App::new(&format!("org.t.App{i}"), &format!("Thing {}", i % 50), "r");
                a.summary = if i % 7 == 0 {
                    "special widget".into()
                } else {
                    String::new()
                };
                a
            })
            .collect();
        let o = opts();
        let big = search(&apps, "widget", &o);
        let expected: Vec<usize> = (0..apps.len()).filter(|i| i % 7 == 0).collect();
        let mut got: Vec<usize> = big.iter().map(|c| c.index).collect();
        got.sort();
        assert_eq!(
            got, expected,
            "every match found exactly once with its global index"
        );
        // Global indexes must point at the right app and ordering stays deterministic.
        assert!(
            big.iter()
                .all(|c| apps[c.index].summary == "special widget")
        );
        assert_eq!(big, search(&apps, "widget", &o));
        let names = search(&apps, "thing 7", &o);
        assert_eq!(
            names.first().map(|c| apps[c.index].name.as_str()),
            Some("Thing 7")
        );
        assert!(
            names
                .iter()
                .filter(|c| c.kind == MatchKind::ExactName)
                .count()
                == apps.len() / 50
        );
    }
}
