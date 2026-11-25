use super::traits::Backend;
use crate::config;
use crate::core::models::{
    Installed, Remote, Scope, Update, installation_flag, is_valid_app_id, is_valid_remote_name,
};
use crate::error::{Error, Result};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Flatpak backend driven through the `flatpak` executable. Arguments are always passed
/// as separate argv entries; nothing goes through a shell.
pub struct FlatpakCli {
    program: OsString,
}

impl FlatpakCli {
    pub fn new(program: impl Into<OsString>) -> Self {
        FlatpakCli {
            program: program.into(),
        }
    }

    /// `IFHF_FLATPAK` overrides the executable (used by tests and benchmarks).
    pub fn from_env() -> Self {
        Self::new(std::env::var_os("IFHF_FLATPAK").unwrap_or_else(|| "flatpak".into()))
    }

    fn missing(&self) -> Error {
        Error::Backend(format!(
            "Flatpak was not found (tried '{}').\n\nInstall Flatpak and make sure it is on your PATH: https://flatpak.org/setup/",
            self.program.to_string_lossy()
        ))
    }

    fn spawn_err(&self, e: std::io::Error) -> Error {
        if e.kind() == std::io::ErrorKind::NotFound {
            self.missing()
        } else {
            Error::Backend(format!("Could not run flatpak: {e}"))
        }
    }

    /// Run and capture stdout; stderr is kept for the error message.
    fn capture(&self, args: &[String]) -> Result<String> {
        let out = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| self.spawn_err(e))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(Error::Backend(format!(
                "flatpak {} failed ({}):\n{}",
                args.first().map(String::as_str).unwrap_or(""),
                out.status,
                err.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Run with the terminal attached so the user sees Flatpak's own progress.
    fn run(&self, args: &[String]) -> Result<()> {
        let status = Command::new(&self.program)
            .args(args)
            .status()
            .map_err(|e| self.spawn_err(e))?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Backend(format!(
                "flatpak {} exited with {status} (see the output above).",
                args.first().map(String::as_str).unwrap_or("")
            )))
        }
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn valid_ref_part(p: &str) -> bool {
    !p.is_empty()
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

/// Accepts a bare application ID or a full `app/ID/ARCH/BRANCH` ref; nothing else reaches Flatpak.
fn check_ids(ids: &[String]) -> Result<()> {
    for id in ids {
        let ok = match id.strip_prefix("app/") {
            Some(rest) => {
                let p: Vec<&str> = rest.split('/').collect();
                p.len() == 3
                    && is_valid_app_id(p[0])
                    && valid_ref_part(p[1])
                    && valid_ref_part(p[2])
            }
            None => is_valid_app_id(id),
        };
        if !ok {
            return Err(Error::Usage(format!(
                "'{id}' is not a valid application ID or ref."
            )));
        }
    }
    Ok(())
}

pub fn install_args(scope: Scope, remote: &str, refs: &[String]) -> Result<Vec<String>> {
    if !is_valid_remote_name(remote) {
        return Err(Error::Usage(format!(
            "'{remote}' is not a valid remote name."
        )));
    }
    check_ids(refs)?;
    let mut a = s(&["install", scope.flag(), "-y", remote]);
    a.extend(refs.iter().cloned());
    Ok(a)
}

pub fn remove_args(installation: &str, ids: &[String], delete_data: bool) -> Result<Vec<String>> {
    check_ids(ids)?;
    let mut a = vec![
        "uninstall".to_string(),
        installation_flag(installation),
        "-y".into(),
    ];
    if delete_data {
        a.push("--delete-data".into());
    }
    a.extend(ids.iter().cloned());
    Ok(a)
}

pub fn update_args(installation: &str, ids: &[String]) -> Result<Vec<String>> {
    check_ids(ids)?;
    let mut a = vec![
        "update".to_string(),
        installation_flag(installation),
        "-y".into(),
    ];
    a.extend(ids.iter().cloned());
    Ok(a)
}

impl Backend for FlatpakCli {
    fn installation_dirs(&self) -> Vec<(Scope, PathBuf)> {
        let mut v = Vec::new();
        if let Some(u) = config::flatpak_user_dir() {
            v.push((Scope::User, u));
        }
        v.push((Scope::System, config::flatpak_system_dir()));
        v
    }

    fn remotes(&self) -> Result<Vec<Remote>> {
        let out = self.capture(&s(&[
            "remotes",
            "--show-disabled",
            "--columns=name,options,url",
        ]))?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                let name = f.next()?.trim();
                let opts = f.next().unwrap_or("");
                let url = f.next().unwrap_or("").trim();
                if name.is_empty() {
                    return None;
                }
                let has = |o: &str| opts.split(',').any(|x| x.trim() == o);
                Some(Remote {
                    name: name.to_string(),
                    url: url.to_string(),
                    scope: if has("user") {
                        Scope::User
                    } else {
                        Scope::System
                    },
                    enabled: !has("disabled"),
                })
            })
            .collect())
    }

    fn installed(&self) -> Result<Vec<Installed>> {
        let out = self.capture(&s(&[
            "list",
            "--columns=ref,name,origin,installation,options",
        ]))?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() < 5 {
                    return None;
                }
                let mut r = f[0].split('/');
                let (id, arch, branch) = (r.next()?, r.next()?, r.next()?);
                Some(Installed {
                    id: id.into(),
                    name: f[1].trim().into(),
                    branch: branch.into(),
                    arch: arch.into(),
                    origin: f[2].trim().into(),
                    installation: f[3].trim().into(),
                    is_runtime: f[4].split(',').any(|o| o.trim() == "runtime"),
                })
            })
            .collect())
    }

    fn install(&self, scope: Scope, remote: &str, refs: &[String]) -> Result<()> {
        self.run(&install_args(scope, remote, refs)?)
    }

    fn remove(&self, installation: &str, ids: &[String], delete_data: bool) -> Result<()> {
        self.run(&remove_args(installation, ids, delete_data)?)
    }

    fn updates(&self) -> Result<Vec<Update>> {
        let mut all = Vec::new();
        // Flatpak answers an installation without remotes with an empty list, so no pre-check.
        for scope in [Scope::User, Scope::System] {
            let out = self.capture(&[
                "remote-ls".into(),
                "--updates".into(),
                scope.flag().into(),
                "--columns=ref,name,origin".into(),
            ])?;
            for l in out.lines() {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() < 3 {
                    continue;
                }
                let mut r = f[0].split('/');
                let (Some(kind), Some(id), Some(_arch), Some(branch)) =
                    (r.next(), r.next(), r.next(), r.next())
                else {
                    continue;
                };
                all.push(Update {
                    id: id.into(),
                    name: f[1].trim().into(),
                    branch: branch.into(),
                    origin: f[2].trim().into(),
                    installation: scope.as_str().into(),
                    is_runtime: kind == "runtime",
                });
            }
        }
        Ok(all)
    }

    fn update(&self, installation: &str, ids: &[String]) -> Result<()> {
        self.run(&update_args(installation, ids)?)
    }

    fn sync(&self) -> Result<()> {
        self.run(&s(&["update", "--appstream"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_argv() {
        let a = install_args(
            Scope::User,
            "flathub",
            &["app/org.a.B/x86_64/stable".into()],
        )
        .unwrap();
        assert_eq!(
            a,
            [
                "install",
                "--user",
                "-y",
                "flathub",
                "app/org.a.B/x86_64/stable"
            ]
        );
        let a = install_args(
            Scope::System,
            "flathub",
            &["org.a.B".into(), "org.c.D".into()],
        )
        .unwrap();
        assert_eq!(
            a,
            ["install", "--system", "-y", "flathub", "org.a.B", "org.c.D"]
        );
    }

    #[test]
    fn remove_argv_preserves_data_by_default() {
        let a = remove_args("system", &["org.a.B".into()], false).unwrap();
        assert_eq!(a, ["uninstall", "--system", "-y", "org.a.B"]);
        let a = remove_args("extra", &["org.a.B".into()], true).unwrap();
        assert_eq!(
            a,
            [
                "uninstall",
                "--installation=extra",
                "-y",
                "--delete-data",
                "org.a.B"
            ]
        );
    }

    #[test]
    fn update_argv() {
        assert_eq!(
            update_args("user", &[]).unwrap(),
            ["update", "--user", "-y"]
        );
        assert_eq!(
            update_args("user", &["org.a.B".into()]).unwrap(),
            ["update", "--user", "-y", "org.a.B"]
        );
    }

    #[test]
    fn hostile_input_is_rejected_before_spawning() {
        assert!(install_args(Scope::User, "--system", &["org.a.B".into()]).is_err());
        assert!(install_args(Scope::User, "flathub", &["--user".into()]).is_err());
        assert!(install_args(Scope::User, "flathub", &["org.a.B; reboot".into()]).is_err());
        assert!(remove_args("user", &["$(id)".into()], false).is_err());
    }

    #[test]
    fn missing_executable_is_reported_clearly() {
        let b = FlatpakCli::new("/nonexistent/flatpak-binary");
        let e = b.remotes().unwrap_err().to_string();
        assert!(e.contains("Flatpak was not found"), "{e}");
    }
}
