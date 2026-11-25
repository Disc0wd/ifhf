//! In-memory backend for tests; never touches the real Flatpak installation.

use super::traits::Backend;
use crate::core::models::{Installed, Remote, Scope, Update};
use crate::error::{Error, Result};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct MockBackend {
    pub dirs: Vec<(Scope, PathBuf)>,
    pub remotes: Vec<Remote>,
    pub installed: Mutex<Vec<Installed>>,
    pub updates: Vec<Update>,
    /// Every mutating call, as a readable string, in order.
    pub calls: Mutex<Vec<String>>,
    /// Installing/removing/updating a ref or id containing one of these fails.
    pub fail_on: Vec<String>,
}

impl MockBackend {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn add_installed(&self, id: &str, name: &str, origin: &str, installation: &str) {
        self.installed.lock().unwrap().push(Installed {
            id: id.into(),
            name: name.into(),
            branch: "stable".into(),
            arch: "x86_64".into(),
            origin: origin.into(),
            installation: installation.into(),
            is_runtime: false,
        });
    }

    fn fails(&self, s: &str) -> bool {
        self.fail_on.iter().any(|f| s.contains(f.as_str()))
    }
}

impl Backend for MockBackend {
    fn installation_dirs(&self) -> Vec<(Scope, PathBuf)> {
        self.dirs.clone()
    }
    fn remotes(&self) -> Result<Vec<Remote>> {
        Ok(self.remotes.clone())
    }
    fn installed(&self, include_runtimes: bool) -> Result<Vec<Installed>> {
        let all = self.installed.lock().unwrap().clone();
        Ok(all
            .into_iter()
            .filter(|i| include_runtimes || !i.is_runtime)
            .collect())
    }
    fn install(&self, scope: Scope, remote: &str, refs: &[String]) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("install {scope} {remote} {}", refs.join(" ")));
        for r in refs {
            if self.fails(r) {
                return Err(Error::Backend(format!("mock: cannot install {r}")));
            }
            let id = r
                .strip_prefix("app/")
                .unwrap_or(r)
                .split('/')
                .next()
                .unwrap()
                .to_string();
            self.add_installed(&id, &id, remote, scope.as_str());
        }
        Ok(())
    }
    fn remove(&self, installation: &str, ids: &[String], delete_data: bool) -> Result<()> {
        self.calls.lock().unwrap().push(format!(
            "remove {installation} {}{}",
            if delete_data { "--delete-data " } else { "" },
            ids.join(" ")
        ));
        for id in ids {
            if self.fails(id) {
                return Err(Error::Backend(format!("mock: cannot remove {id}")));
            }
            self.installed
                .lock()
                .unwrap()
                .retain(|i| !(i.id == *id && i.installation == installation));
        }
        Ok(())
    }
    fn updates(&self) -> Result<Vec<Update>> {
        Ok(self.updates.clone())
    }
    fn update(&self, installation: &str, ids: &[String]) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("update {installation} {}", ids.join(" ")));
        if ids.iter().any(|i| self.fails(i)) {
            return Err(Error::Backend("mock: update failed".into()));
        }
        Ok(())
    }
    fn sync(&self) -> Result<()> {
        self.calls.lock().unwrap().push("sync".into());
        Ok(())
    }
}
