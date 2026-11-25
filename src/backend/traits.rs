use crate::core::models::{Installed, Remote, Scope, Update};
use crate::error::Result;
use std::path::{Path, PathBuf};

/// An AppStream metadata file belonging to one remote of one installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub remote: String,
    pub scope: Scope,
    pub arch: String,
    pub path: PathBuf,
}

/// Everything IFHF needs from Flatpak. Nothing outside `backend` runs Flatpak commands.
pub trait Backend {
    /// Flatpak installation directories (user, system) whose metadata we read.
    fn installation_dirs(&self) -> Vec<(Scope, PathBuf)>;
    fn remotes(&self) -> Result<Vec<Remote>>;
    fn installed(&self) -> Result<Vec<Installed>>;
    /// `refs` are full refs or IDs; all come from one remote in one scope (one transaction).
    fn install(&self, scope: Scope, remote: &str, refs: &[String]) -> Result<()>;
    fn remove(&self, installation: &str, ids: &[String], delete_data: bool) -> Result<()>;
    fn updates(&self) -> Result<Vec<Update>>;
    /// Empty `ids` updates everything in that installation.
    fn update(&self, installation: &str, ids: &[String]) -> Result<()>;
    /// Ask Flatpak to refresh remote AppStream metadata.
    fn sync(&self) -> Result<()>;

    /// Locate AppStream files for enabled remotes. Only runs on a cache miss.
    fn appstream_sources(&self) -> Result<Vec<Source>> {
        let remotes = self.remotes()?;
        let host = host_arch();
        let mut out = Vec::new();
        for (scope, dir) in self.installation_dirs() {
            for r in remotes.iter().filter(|r| r.enabled && r.scope == scope) {
                let base = dir.join("appstream").join(&r.name);
                let Ok(arches) = std::fs::read_dir(&base) else {
                    continue;
                };
                let mut arches: Vec<_> = arches.flatten().collect();
                arches.sort_by_key(|e| e.file_name());
                for a in arches {
                    let arch = a.file_name().to_string_lossy().into_owned();
                    if arch != host {
                        continue;
                    }
                    if let Some(path) = metadata_file(&a.path()) {
                        out.push(Source {
                            remote: r.name.clone(),
                            scope,
                            arch,
                            path,
                        });
                    }
                }
            }
        }
        Ok(out)
    }
}

/// Flatpak's name for the host architecture.
pub fn host_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "i386",
        a => a,
    }
}

/// `active/appstream.xml` is preferred over compressed copies; OCI remotes ship only `.xml.gz`.
fn metadata_file(arch_dir: &Path) -> Option<PathBuf> {
    [
        "active/appstream.xml",
        "active/appstream.xml.gz",
        "appstream.xml",
        "appstream.xml.gz",
    ]
    .iter()
    .map(|p| arch_dir.join(p))
    .find(|p| p.is_file())
}
