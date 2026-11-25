//! Disposable text index of the parsed catalogue.
//!
//! Layout: a small header (format, build time, installation dirs, file stamps) followed by
//! one tab-separated record per application. It is written to a temp file and renamed into
//! place, so a failed refresh never damages the last known-good index.

use crate::core::models::{App, Scope};
use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The IFHF version is part of the header: a new release may parse metadata differently, so an
/// index written by another version is ignored and rebuilt.
const MAGIC: &str = concat!("IFHF-INDEX\t1\t", env!("CARGO_PKG_VERSION"));
const KW_SEP: char = '\u{1f}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrcRec {
    pub remote: String,
    pub path: String,
    pub stamp: String,
    /// Unix time the metadata was last refreshed by Flatpak.
    pub synced: u64,
}

#[derive(Debug)]
pub struct Index {
    pub built: u64,
    pub dirs: Vec<(Scope, PathBuf)>,
    /// (path, stamp) of files/dirs whose change means remotes were added/removed/enabled.
    pub watch: Vec<(String, String)>,
    pub sources: Vec<SrcRec>,
    pub apps: Vec<App>,
}

fn mtime_ns(m: &fs::Metadata) -> u128 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Identity of a metadata file: resolved path (changes when Flatpak swaps `active`),
/// size, and the mtime of its directory (Flatpak refreshes the whole directory).
pub fn stamp_file(path: &Path) -> String {
    let Ok(canon) = fs::canonicalize(path) else {
        return "missing".into();
    };
    let Ok(m) = fs::metadata(&canon) else {
        return "missing".into();
    };
    let dir = canon
        .parent()
        .and_then(|d| fs::metadata(d).ok())
        .map(|d| mtime_ns(&d))
        .unwrap_or(0);
    format!("{}|{}|{}", canon.display(), m.len(), dir)
}

pub fn stamp_path(path: &Path) -> String {
    fs::metadata(path)
        .map(|m| mtime_ns(&m).to_string())
        .unwrap_or_else(|_| "missing".into())
}

pub fn synced_secs(path: &Path) -> u64 {
    fs::canonicalize(path)
        .ok()
        .and_then(|c| c.parent().and_then(|d| fs::metadata(d).ok()))
        .map(|m| (mtime_ns(&m) / 1_000_000_000) as u64)
        .unwrap_or(0)
}

pub fn watch_stamps(dirs: &[(Scope, PathBuf)]) -> Vec<(String, String)> {
    dirs.iter()
        .flat_map(|(_, d)| [d.join("appstream"), d.join("repo").join("config")])
        .map(|p| {
            let s = stamp_path(&p);
            (p.display().to_string(), s)
        })
        .collect()
}

pub fn is_fresh(idx: &Index, dirs: &[(Scope, PathBuf)], max_age_hours: u64, now: u64) -> bool {
    idx.dirs == dirs
        && now.saturating_sub(idx.built) < max_age_hours.saturating_mul(3600)
        && idx
            .watch
            .iter()
            .all(|(p, s)| stamp_path(Path::new(p)) == *s)
        && idx
            .sources
            .iter()
            .all(|r| stamp_file(Path::new(&r.path)) == r.stamp)
}

fn esc(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
}

fn unesc(s: &str) -> Cow<'_, str> {
    if !s.contains('\\') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    Cow::Owned(out)
}

fn scope_code(s: Scope) -> &'static str {
    match s {
        Scope::User => "u",
        Scope::System => "s",
    }
}

pub fn write(path: &Path, idx: &Index) -> io::Result<()> {
    let mut o = String::with_capacity(idx.apps.len() * 600 + 1024);
    let _ = writeln!(o, "{MAGIC}");
    let _ = writeln!(o, "built\t{}", idx.built);
    for (s, d) in &idx.dirs {
        let _ = write!(o, "dir\t{}\t", scope_code(*s));
        esc(&d.display().to_string(), &mut o);
        o.push('\n');
    }
    for (p, s) in &idx.watch {
        o.push_str("watch\t");
        esc(p, &mut o);
        o.push('\t');
        esc(s, &mut o);
        o.push('\n');
    }
    for r in &idx.sources {
        o.push_str("src\t");
        esc(&r.remote, &mut o);
        o.push('\t');
        esc(&r.path, &mut o);
        o.push('\t');
        esc(&r.stamp, &mut o);
        let _ = writeln!(o, "\t{}", r.synced);
    }
    o.push_str("apps\n");
    for a in &idx.apps {
        let kw = a.keywords.join(&KW_SEP.to_string());
        let fields: [&str; 10] = [
            &a.id,
            &a.name,
            &a.summary,
            &a.description,
            &kw,
            a.homepage.as_deref().unwrap_or(""),
            a.developer.as_deref().unwrap_or(""),
            &a.remote,
            a.branch.as_deref().unwrap_or(""),
            a.arch.as_deref().unwrap_or(""),
        ];
        for f in fields {
            esc(f, &mut o);
            o.push('\t');
        }
        o.push_str(scope_code(a.scope));
        o.push('\n');
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".index.tsv.{}.tmp", std::process::id()));
    fs::write(&tmp, o)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn parse_record(line: &str) -> Option<App> {
    let mut f = line.split('\t');
    let mut next = || f.next().map(unesc);
    let id = next()?.into_owned();
    let name = next()?.into_owned();
    let summary = next()?.into_owned();
    let description = next()?.into_owned();
    let kw = next()?;
    let homepage = next()?.into_owned();
    let developer = next()?.into_owned();
    let remote = next()?.into_owned();
    let branch = next()?.into_owned();
    let arch = next()?.into_owned();
    let scope = next()?;
    let opt = |s: String| if s.is_empty() { None } else { Some(s) };
    Some(App {
        id,
        name,
        summary,
        description,
        keywords: kw
            .split(KW_SEP)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
            .collect(),
        homepage: opt(homepage),
        developer: opt(developer),
        remote,
        branch: opt(branch),
        arch: opt(arch),
        scope: if scope == "u" {
            Scope::User
        } else {
            Scope::System
        },
    })
}

/// Below this many bytes a single thread is faster than spawning workers.
const PARALLEL_MIN_BYTES: usize = 256 * 1024;

/// Parse the record section, in parallel for large indexes. Chunks split on line boundaries and
/// are rejoined in order, so the result is identical to a sequential parse.
fn parse_records(body: &str) -> Option<Vec<App>> {
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8);
    if workers < 2 || body.len() < PARALLEL_MIN_BYTES {
        return body.lines().map(parse_record).collect();
    }
    let mut cuts = vec![0];
    for i in 1..workers {
        let guess = body.len() * i / workers;
        let at = body[guess..]
            .find('\n')
            .map_or(body.len(), |n| guess + n + 1);
        if at > *cuts.last().unwrap() {
            cuts.push(at);
        }
    }
    cuts.push(body.len());
    cuts.dedup();
    let parts: Vec<Option<Vec<App>>> = std::thread::scope(|s| {
        let handles: Vec<_> = cuts
            .windows(2)
            .map(|w| {
                let chunk = &body[w[0]..w[1]];
                s.spawn(move || {
                    chunk
                        .lines()
                        .map(parse_record)
                        .collect::<Option<Vec<App>>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().ok().flatten())
            .collect()
    });
    let mut apps = Vec::new();
    for p in parts {
        apps.extend(p?);
    }
    Some(apps)
}

/// None for a missing, truncated or foreign file: the caller simply rebuilds.
pub fn read(path: &Path) -> Option<Index> {
    let text = fs::read_to_string(path).ok()?;
    let split = text.find("\napps\n")? + "\napps\n".len();
    let (header, body) = text.split_at(split);
    let mut lines = header.lines();
    if lines.next()? != MAGIC {
        return None;
    }
    let mut idx = Index {
        built: 0,
        dirs: vec![],
        watch: vec![],
        sources: vec![],
        apps: vec![],
    };
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "built" => idx.built = f.get(1)?.parse().ok()?,
            "dir" => {
                let s = if *f.get(1)? == "u" {
                    Scope::User
                } else {
                    Scope::System
                };
                idx.dirs.push((s, PathBuf::from(unesc(f.get(2)?).as_ref())));
            }
            "watch" => idx
                .watch
                .push((unesc(f.get(1)?).into_owned(), unesc(f.get(2)?).into_owned())),
            "src" => idx.sources.push(SrcRec {
                remote: unesc(f.get(1)?).into_owned(),
                path: unesc(f.get(2)?).into_owned(),
                stamp: unesc(f.get(3)?).into_owned(),
                synced: f.get(4)?.parse().ok()?,
            }),
            "apps" => {}
            _ => return None,
        }
    }
    idx.apps = parse_records(body)?;
    Some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Index {
        let mut a = App::new("org.a.B", "Bee\twith\ttabs", "flathub");
        a.description = "line1\nline2 \\ backslash".into();
        a.keywords = vec!["x".into(), "y z".into()];
        a.homepage = Some("https://x".into());
        a.branch = Some("stable".into());
        a.arch = Some("x86_64".into());
        a.scope = Scope::System;
        let b = App::new("org.c.D", "", "r");
        Index {
            built: 42,
            dirs: vec![(Scope::User, "/u".into()), (Scope::System, "/s dir".into())],
            watch: vec![("/s/appstream".into(), "123".into())],
            sources: vec![SrcRec {
                remote: "flathub".into(),
                path: "/p".into(),
                stamp: "a|1|2".into(),
                synced: 7,
            }],
            apps: vec![a, b],
        }
    }

    #[test]
    fn round_trip_preserves_everything() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c/index.tsv");
        let idx = sample();
        write(&p, &idx).unwrap();
        let back = read(&p).unwrap();
        assert_eq!(back.built, 42);
        assert_eq!(back.dirs, idx.dirs);
        assert_eq!(back.watch, idx.watch);
        assert_eq!(back.sources, idx.sources);
        assert_eq!(back.apps, idx.apps);
    }

    #[test]
    fn index_from_another_version_is_ignored() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("i");
        write(&p, &sample()).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        let old = text.replacen(env!("CARGO_PKG_VERSION"), "0.0.0-old", 1);
        fs::write(&p, old).unwrap();
        assert!(
            read(&p).is_none(),
            "an index written by a different version must be rebuilt"
        );
    }

    #[test]
    fn corrupt_or_foreign_files_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("i");
        assert!(read(&p).is_none());
        fs::write(&p, "garbage").unwrap();
        assert!(read(&p).is_none());
        let good = {
            write(&p, &sample()).unwrap();
            fs::read_to_string(&p).unwrap()
        };
        fs::write(&p, &good[..good.len() - 10]).unwrap(); // truncated mid-record
        assert!(read(&p).is_none());
        fs::write(&p, good.replace("apps\n", "")).unwrap(); // header never completes
        assert!(read(&p).is_none());
    }

    #[test]
    fn freshness_rules() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("m.xml");
        fs::write(&f, "x").unwrap();
        let dirs = vec![(Scope::System, d.path().to_path_buf())];
        let mut idx = Index {
            built: 1000,
            dirs: dirs.clone(),
            watch: watch_stamps(&dirs),
            sources: vec![SrcRec {
                remote: "r".into(),
                path: f.display().to_string(),
                stamp: stamp_file(&f),
                synced: 0,
            }],
            apps: vec![],
        };
        assert!(is_fresh(&idx, &dirs, 24, 1000 + 3600));
        assert!(!is_fresh(&idx, &dirs, 24, 1000 + 25 * 3600), "too old");
        assert!(
            !is_fresh(&idx, &dirs, 0, 1000),
            "max_age 0 always refreshes"
        );
        assert!(
            !is_fresh(&idx, &[(Scope::User, d.path().to_path_buf())], 24, 1001),
            "dirs changed"
        );
        fs::write(&f, "changed contents").unwrap();
        assert!(!is_fresh(&idx, &dirs, 24, 1001), "source changed");
        idx.sources[0].stamp = stamp_file(&f);
        assert!(is_fresh(&idx, &dirs, 24, 1001));
        fs::remove_file(&f).unwrap();
        assert!(!is_fresh(&idx, &dirs, 24, 1001), "source removed");
    }

    #[test]
    fn failed_write_keeps_previous_index() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("index.tsv");
        write(&p, &sample()).unwrap();
        // Make the target a directory so the rename fails.
        let blocked = d.path().join("blocked");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("keep"), "x").unwrap();
        assert!(write(&blocked, &sample()).is_err());
        assert!(read(&p).is_some());
        let leftovers: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn large_indexes_parse_in_parallel_identically_to_sequential() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("index.tsv");
        let mut idx = sample();
        idx.apps = (0..4000)
            .map(|i| {
                let mut a = App::new(&format!("org.test.App{i}"), &format!("App {i}"), "flathub");
                a.description = format!("tab\there {i} {}", "word ".repeat(40));
                a.keywords = vec![format!("kw{i}")];
                a
            })
            .collect();
        write(&p, &idx).unwrap();
        let body_len = fs::metadata(&p).unwrap().len() as usize;
        assert!(
            body_len > PARALLEL_MIN_BYTES,
            "fixture must exceed the parallel threshold"
        );
        let back = read(&p).unwrap();
        assert_eq!(back.apps, idx.apps, "order and content preserved");
        let text = fs::read_to_string(&p).unwrap();
        let body = &text[text.find("\napps\n").unwrap() + 6..];
        let sequential: Vec<App> = body.lines().map(|l| parse_record(l).unwrap()).collect();
        assert_eq!(back.apps, sequential);
    }
}
