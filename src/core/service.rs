//! Executes already-confirmed operations and reports exactly what succeeded and failed.
//! Flatpak stays responsible for the actual work; nothing here claims atomicity.

use super::models::{App, Installed, Scope};
use crate::backend::Backend;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct OpReport {
    pub done: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub fn label(app: &App) -> String {
    if app.name.is_empty() {
        app.id.clone()
    } else {
        app.name.clone()
    }
}

/// One Flatpak transaction per (scope, remote). If a transaction fails, the installed list
/// decides which of its applications actually made it.
pub fn install(backend: &dyn Backend, items: &[App]) -> OpReport {
    let mut groups: BTreeMap<(&str, &str), Vec<&App>> = BTreeMap::new();
    for a in items {
        groups
            .entry((a.scope.as_str(), a.remote.as_str()))
            .or_default()
            .push(a);
    }
    let mut rep = OpReport::default();
    for ((scope, remote), apps) in groups {
        let scope = if scope == "user" {
            Scope::User
        } else {
            Scope::System
        };
        let refs: Vec<String> = apps.iter().map(|a| a.flatpak_ref()).collect();
        match backend.install(scope, remote, &refs) {
            Ok(()) => rep.done.extend(apps.iter().map(|a| label(a))),
            Err(e) => {
                let now = backend.installed(false).ok();
                for a in apps {
                    let present = now.as_ref().is_some_and(|l| {
                        l.iter()
                            .any(|i| i.id == a.id && i.installation == scope.as_str())
                    });
                    if present {
                        rep.done.push(label(a));
                    } else {
                        rep.failed.push((label(a), e.to_string()));
                    }
                }
            }
        }
    }
    rep
}

pub fn remove(backend: &dyn Backend, items: &[Installed], delete_data: bool) -> OpReport {
    let mut groups: BTreeMap<&str, Vec<&Installed>> = BTreeMap::new();
    for i in items {
        groups.entry(i.installation.as_str()).or_default().push(i);
    }
    let mut rep = OpReport::default();
    let name = |i: &Installed| {
        if i.name.is_empty() {
            i.id.clone()
        } else {
            i.name.clone()
        }
    };
    for (inst, apps) in groups {
        let ids: Vec<String> = apps.iter().map(|a| a.id.clone()).collect();
        match backend.remove(inst, &ids, delete_data) {
            Ok(()) => rep.done.extend(apps.iter().map(|a| name(a))),
            Err(e) => {
                let now = backend.installed(true).ok();
                for a in apps {
                    let gone = now
                        .as_ref()
                        .is_some_and(|l| !l.iter().any(|i| i.id == a.id && i.installation == inst));
                    if gone {
                        rep.done.push(name(a));
                    } else {
                        rep.failed.push((name(a), e.to_string()));
                    }
                }
            }
        }
    }
    rep
}

/// `targets`: (installation, ids-with-labels). Empty id list updates the whole installation.
pub fn update(backend: &dyn Backend, targets: &[(String, Vec<(String, String)>)]) -> OpReport {
    let mut rep = OpReport::default();
    for (inst, apps) in targets {
        let ids: Vec<String> = apps.iter().map(|(id, _)| id.clone()).collect();
        let names: Vec<String> = if apps.is_empty() {
            vec![format!("all updates ({inst})")]
        } else {
            apps.iter().map(|(_, n)| n.clone()).collect()
        };
        match backend.update(inst, &ids) {
            Ok(()) => rep.done.extend(names),
            Err(e) => rep
                .failed
                .extend(names.into_iter().map(|n| (n, e.to_string()))),
        }
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    fn app(id: &str, remote: &str, scope: Scope) -> App {
        let mut a = App::new(id, id, remote);
        a.scope = scope;
        a
    }

    #[test]
    fn install_groups_by_scope_and_remote() {
        let b = MockBackend::default();
        let rep = install(
            &b,
            &[
                app("org.a.A", "flathub", Scope::User),
                app("org.b.B", "flathub", Scope::User),
                app("org.c.C", "other", Scope::User),
            ],
        );
        assert_eq!(rep.done.len(), 3);
        assert_eq!(
            b.calls(),
            vec![
                "install user flathub org.a.A org.b.B",
                "install user other org.c.C"
            ]
        );
    }

    #[test]
    fn partial_install_failure_is_exact() {
        let b = MockBackend {
            fail_on: vec!["org.b.B".into()],
            ..MockBackend::default()
        };
        let rep = install(
            &b,
            &[
                app("org.a.A", "flathub", Scope::User),
                app("org.b.B", "flathub", Scope::User),
            ],
        );
        assert_eq!(rep.done, vec!["org.a.A"]);
        assert_eq!(rep.failed.len(), 1);
        assert_eq!(rep.failed[0].0, "org.b.B");
    }

    #[test]
    fn remove_reports_per_application() {
        let b = MockBackend {
            fail_on: vec!["org.b.B".into()],
            ..MockBackend::default()
        };
        b.add_installed("org.a.A", "A", "flathub", "system");
        b.add_installed("org.b.B", "B", "flathub", "system");
        let items = b.installed(true).unwrap();
        let rep = remove(&b, &items, false);
        assert_eq!(rep.done, vec!["A"]);
        assert_eq!(rep.failed[0].0, "B");
        assert!(b.calls()[0].starts_with("remove system org.a.A org.b.B"));
    }

    #[test]
    fn delete_data_flag_is_forwarded_only_when_asked() {
        let b = MockBackend::default();
        b.add_installed("org.a.A", "A", "flathub", "user");
        let items = b.installed(true).unwrap();
        remove(&b, &items, true);
        assert_eq!(b.calls(), vec!["remove user --delete-data org.a.A"]);
    }
}
