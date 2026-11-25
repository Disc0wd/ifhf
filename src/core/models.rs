use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    User,
    System,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::System => "system",
        }
    }
    pub fn flag(self) -> &'static str {
        match self {
            Scope::User => "--user",
            Scope::System => "--system",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Flatpak flag selecting an installation by the name `flatpak list` reports.
pub fn installation_flag(installation: &str) -> String {
    match installation {
        "user" => "--user".into(),
        "system" => "--system".into(),
        other => format!("--installation={other}"),
    }
}

/// An application available from a remote's AppStream metadata (or an installed one,
/// converted for resolution). Missing metadata stays empty/None, never fabricated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct App {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub homepage: Option<String>,
    pub developer: Option<String>,
    pub remote: String,
    pub branch: Option<String>,
    pub arch: Option<String>,
    pub scope: Scope,
}

impl App {
    pub fn new(id: &str, name: &str, remote: &str) -> App {
        App {
            id: id.into(),
            name: name.into(),
            remote: remote.into(),
            ..App::default()
        }
    }

    /// Full ref when arch and branch are known, otherwise just the ID.
    pub fn flatpak_ref(&self) -> String {
        match (&self.arch, &self.branch) {
            (Some(a), Some(b)) => format!("app/{}/{}/{}", self.id, a, b),
            _ => self.id.clone(),
        }
    }

    /// What distinguishes two candidates for the same query.
    pub fn identity(&self) -> (&str, &str, Scope, Option<&str>) {
        (&self.id, &self.remote, self.scope, self.branch.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub arch: String,
    pub origin: String,
    /// "user", "system" or a named system installation.
    pub installation: String,
    pub is_runtime: bool,
}

impl Installed {
    pub fn scope(&self) -> Scope {
        if self.installation == "user" {
            Scope::User
        } else {
            Scope::System
        }
    }

    pub fn to_app(&self) -> App {
        App {
            id: self.id.clone(),
            name: self.name.clone(),
            remote: self.origin.clone(),
            branch: Some(self.branch.clone()),
            arch: Some(self.arch.clone()),
            scope: self.scope(),
            ..App::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
    pub scope: Scope,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub origin: String,
    pub installation: String,
    pub is_runtime: bool,
}

/// Conservative check applied to every ID that reaches a Flatpak command line.
pub fn is_valid_app_id(s: &str) -> bool {
    let segs: Vec<&str> = s.split('.').collect();
    segs.len() >= 2
        && segs.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        && !s.starts_with('-')
}

pub fn is_valid_remote_name(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids() {
        assert!(is_valid_app_id("org.mozilla.firefox"));
        assert!(is_valid_app_id("io.github.foo-bar.Baz_1"));
        assert!(!is_valid_app_id("firefox"));
        assert!(!is_valid_app_id("-org.evil"));
        assert!(!is_valid_app_id("org..x"));
        assert!(!is_valid_app_id("org.x; rm -rf"));
        assert!(!is_valid_app_id(""));
    }

    #[test]
    fn remotes() {
        assert!(is_valid_remote_name("flathub"));
        assert!(is_valid_remote_name("flathub-beta"));
        assert!(!is_valid_remote_name("--user"));
        assert!(!is_valid_remote_name("a b"));
        assert!(!is_valid_remote_name(""));
    }

    #[test]
    fn refs_and_flags() {
        let mut a = App::new("org.a.B", "B", "r");
        assert_eq!(a.flatpak_ref(), "org.a.B");
        a.arch = Some("x86_64".into());
        a.branch = Some("stable".into());
        assert_eq!(a.flatpak_ref(), "app/org.a.B/x86_64/stable");
        assert_eq!(installation_flag("user"), "--user");
        assert_eq!(installation_flag("extra"), "--installation=extra");
    }
}
