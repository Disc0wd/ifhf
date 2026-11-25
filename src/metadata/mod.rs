//! AppStream catalogue: parse once, keep a disposable on-disk index, reuse it while the
//! source files are unchanged.

pub mod appstream;
pub mod cache;

use crate::backend::Backend;
use crate::config::Config;
use crate::core::models::App;
use crate::error::{Error, Result};
use std::collections::HashSet;
use std::path::Path;

/// Metadata older than this is flagged as possibly stale (FR-028).
const STALE_AFTER_SECS: u64 = 7 * 24 * 3600;

#[derive(Debug, Default)]
pub struct Catalog {
    pub apps: Vec<App>,
    /// Human-readable warnings (stale or unreadable metadata) for the caller to show.
    pub notes: Vec<String>,
    pub from_cache: bool,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stale_notes(sources: &[cache::SrcRec], now: u64) -> Vec<String> {
    sources
        .iter()
        .filter(|s| now.saturating_sub(s.synced) > STALE_AFTER_SECS)
        .map(|s| {
            format!(
                "Metadata for remote '{}' was last refreshed {} days ago and may be out of date. Run: ifhf sync",
                s.remote,
                now.saturating_sub(s.synced) / 86400
            )
        })
        .collect()
}

pub fn load(backend: &dyn Backend, cfg: &Config, cache_dir: Option<&Path>) -> Result<Catalog> {
    let now = now_secs();
    let dirs = backend.installation_dirs();
    let index_path = cache_dir
        .filter(|_| cfg.cache.enabled)
        .map(|d| d.join("index.tsv"));

    if let Some(p) = &index_path
        && let Some(idx) = cache::read(p)
        && cache::is_fresh(&idx, &dirs, cfg.cache.max_age_hours, now)
    {
        return Ok(Catalog {
            notes: stale_notes(&idx.sources, now),
            apps: idx.apps,
            from_cache: true,
        });
    }
    rebuild(backend, &dirs, index_path.as_deref(), now)
}

/// Re-read the source files and replace the index atomically. A failure leaves the old index.
pub fn rebuild(
    backend: &dyn Backend,
    dirs: &[(crate::core::models::Scope, std::path::PathBuf)],
    index_path: Option<&Path>,
    now: u64,
) -> Result<Catalog> {
    let sources = backend.appstream_sources()?;
    if sources.is_empty() {
        return Err(Error::Metadata(
            "No application metadata was found for any enabled remote.\n\nRefresh it with:\n  ifhf sync\n\nor inspect configured remotes:\n  ifhf remotes"
                .into(),
        ));
    }
    let mut notes = Vec::new();
    let mut apps: Vec<App> = Vec::new();
    let mut recs = Vec::new();
    let mut seen = HashSet::new();
    let mut readable = 0;
    for src in &sources {
        match appstream::load_file(&src.path, &src.remote, src.scope, &src.arch) {
            Ok((list, skipped)) => {
                readable += 1;
                if skipped > 0 {
                    notes.push(format!(
                        "Skipped {skipped} invalid application record(s) from remote '{}'.",
                        src.remote
                    ));
                }
                for a in list {
                    let key = (
                        a.id.clone(),
                        a.remote.clone(),
                        a.scope,
                        a.branch.clone(),
                        a.arch.clone(),
                    );
                    if seen.insert(key) {
                        apps.push(a);
                    }
                }
                recs.push(cache::SrcRec {
                    remote: src.remote.clone(),
                    path: src.path.display().to_string(),
                    stamp: cache::stamp_file(&src.path),
                    synced: cache::synced_secs(&src.path),
                });
            }
            Err(e) => notes.push(format!(
                "Could not read metadata for remote '{}': {e}. Run: ifhf sync",
                src.remote
            )),
        }
    }
    if readable == 0 {
        // Never present a failed refresh as an empty catalogue (FR-015).
        return Err(Error::Metadata(format!(
            "Application metadata could not be read.\n\n{}",
            notes.join("\n")
        )));
    }
    notes.extend(stale_notes(&recs, now));
    if let Some(p) = index_path {
        let idx = cache::Index {
            built: now,
            dirs: dirs.to_vec(),
            watch: cache::watch_stamps(dirs),
            sources: recs,
            apps,
        };
        let _ = cache::write(p, &idx); // best effort: the index is only a cache
        return Ok(Catalog {
            apps: idx.apps,
            notes,
            from_cache: false,
        });
    }
    Ok(Catalog {
        apps,
        notes,
        from_cache: false,
    })
}

/// Rebuild unconditionally (used by `sync`, after Flatpak refreshed the source files).
pub fn refresh(backend: &dyn Backend, cfg: &Config, cache_dir: Option<&Path>) -> Result<Catalog> {
    let dirs = backend.installation_dirs();
    let index_path = cache_dir
        .filter(|_| cfg.cache.enabled)
        .map(|d| d.join("index.tsv"));
    rebuild(backend, &dirs, index_path.as_deref(), now_secs())
}
