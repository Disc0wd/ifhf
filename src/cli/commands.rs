use super::Env;
use super::args::{Command, HELP, Invocation, Opts};
use crate::core::models::{App, Installed, Update};
use crate::core::resolver::{
    self, Candidate, MatchKind, ResolutionResult, ResolutionStatus, SearchOpts,
};
use crate::core::service::{self, OpReport};
use crate::error::{Error, Result};
use crate::metadata::{self, Catalog};
use crate::output::{self, PlanLine, Style};

const DEFAULT_LIMIT: usize = 25;
const MAX_CHOICES: usize = 10;

pub fn execute(env: &mut Env, inv: Invocation) -> Result<()> {
    let Invocation { cmd, opts } = inv;
    match cmd {
        Command::Help => {
            env.io.out(HELP);
            Ok(())
        }
        Command::Version => {
            env.io.out(&format!("ifhf {}\n", env!("CARGO_PKG_VERSION")));
            Ok(())
        }
        Command::Install(q) => install(env, &opts, &q),
        Command::Remove(q) => remove(env, &opts, &q),
        Command::Search(q) => search(env, &opts, &q),
        Command::Info(q) => info(env, &opts, &q),
        Command::List => list(env, &opts),
        Command::Updates => updates(env),
        Command::Update(q) => update(env, &opts, &q),
        Command::Sync => sync(env),
        Command::Remotes => remotes(env),
    }
}

fn style(env: &Env, opts: &Opts) -> Style {
    Style {
        color: env.config.general.color && !opts.no_color && env.io.color_allowed(),
    }
}

fn load_catalog(env: &mut Env) -> Result<Catalog> {
    let cat = metadata::load(env.backend, env.config, env.cache_dir.as_deref())?;
    for n in &cat.notes {
        env.io.err(&format!("Note: {n}\n"));
    }
    Ok(cat)
}

fn search_opts(
    env: &Env,
    remote: Option<String>,
    scope: Option<crate::core::models::Scope>,
) -> SearchOpts {
    SearchOpts {
        fuzzy: env.config.search.fuzzy_matching,
        descriptions: env.config.search.include_descriptions,
        remote,
        scope,
    }
}

/// `--remote` wins over the configured default remote.
fn effective_remote(env: &Env, opts: &Opts) -> Option<String> {
    opts.remote
        .clone()
        .or_else(|| Some(env.config.general.default_remote.clone()).filter(|r| !r.is_empty()))
}

/// Ask `[Y/n]`; skipped with --yes or when the config disables this confirmation.
fn confirm(env: &mut Env, question: &str, default_yes: bool, skip: bool) -> Result<()> {
    if skip {
        return Ok(());
    }
    if !env.io.interactive() {
        return Err(Error::NeedsConfirmation(
            "Confirmation is required but input is not a terminal.\n\nRe-run with --yes to proceed without asking.".into(),
        ));
    }
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    match env
        .io
        .ask(&format!("{question} {hint} "))
        .as_deref()
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("") => {
            if default_yes {
                Ok(())
            } else {
                Err(Error::Aborted)
            }
        }
        Some("y") | Some("yes") => Ok(()),
        _ => Err(Error::Aborted),
    }
}

/// Let the user choose between similar matches. Never picks one on its own.
fn pick(env: &mut Env, apps: &[App], res: &ResolutionResult, st: Style) -> Result<usize> {
    let shown: &[Candidate] = &res.candidates[..res.candidates.len().min(MAX_CHOICES)];
    let list = output::candidate_list(apps, shown, st);
    let hidden = res.candidates.len() - shown.len();
    let more = if hidden > 0 {
        format!("({hidden} more not shown; be more specific.)\n")
    } else {
        String::new()
    };
    let head = if res.candidates.len() > 1 {
        format!("Multiple applications match '{}':", res.query)
    } else {
        format!("'{}' is not an exact match. Closest result:", res.query)
    };
    if !env.io.interactive() {
        return Err(Error::Ambiguous(format!(
            "{head}\n\n{list}{more}Specify an application ID, or narrow it with --remote / --user / --system."
        )));
    }
    env.io.out(&format!("{head}\n\n{list}{more}"));
    for _ in 0..3 {
        let prompt = format!("Choose a result [1-{}, q to cancel]: ", shown.len());
        let Some(ans) = env.io.ask(&prompt) else {
            break;
        };
        if ans.eq_ignore_ascii_case("q") || ans.is_empty() {
            return Err(Error::Aborted);
        }
        if let Ok(n) = ans.parse::<usize>()
            && (1..=shown.len()).contains(&n)
        {
            return Ok(shown[n - 1].index);
        }
        env.io.out("Not a valid choice.\n");
    }
    Err(Error::Aborted)
}

/// Pick the application for a resolved query. `--yes` never accepts a fuzzy-only match:
/// anything below a normalised exact name has to be chosen explicitly.
fn choose(
    env: &mut Env,
    apps: &[App],
    res: &ResolutionResult,
    st: Style,
    yes: bool,
) -> Result<usize> {
    let weak = res.candidates[0].kind < MatchKind::NormalizedName;
    if res.status == ResolutionStatus::Ambiguous || (yes && weak) {
        pick(env, apps, res, st)
    } else {
        Ok(res.candidates[0].index)
    }
}

/// Several problems are reported together; the first decides the exit code.
fn combine(mut problems: Vec<Error>) -> Error {
    if problems.len() == 1 {
        return problems.remove(0);
    }
    let msg = problems
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    let code_source = &problems[0];
    match code_source {
        Error::NotFound(_) => Error::NotFound(msg),
        Error::Ambiguous(_) => Error::Ambiguous(msg),
        Error::Aborted => Error::Aborted,
        _ => Error::Partial(msg),
    }
}

fn not_found(q: &str, remote: &Option<String>) -> Error {
    let r = remote
        .as_ref()
        .map(|r| format!(" in remote '{r}'"))
        .unwrap_or_default();
    Error::NotFound(format!(
        "No application matches '{q}'{r}.\n\nTry searching:\n  ifhf search {q}\nor refresh metadata:\n  ifhf sync"
    ))
}

fn report(env: &mut Env, verb: &str, action: &str, rep: &OpReport) -> Result<()> {
    if !rep.done.is_empty() {
        env.io.out(&format!("{verb}: {}\n", rep.done.join(", ")));
    }
    if rep.failed.is_empty() {
        return Ok(());
    }
    let mut m = String::new();
    for (n, e) in &rep.failed {
        m.push_str(&format!("Could not {action} {n}.\n{e}\n\n"));
    }
    m.push_str("Check your network connection or inspect configured remotes:\n  ifhf remotes");
    if rep.done.is_empty() {
        Err(Error::Backend(m))
    } else {
        Err(Error::Partial(format!(
            "Partly completed. Succeeded: {}. Failed: {}.\n\n{m}",
            rep.done.join(", "),
            rep.failed
                .iter()
                .map(|f| f.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

fn install(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let cat = load_catalog(env)?;
    let remote = effective_remote(env, opts);
    let want = opts.scope.unwrap_or(env.config.general.default_scope);
    let explicit = opts.scope.is_some();

    let mut chosen: Vec<App> = Vec::new();
    let mut problems = Vec::new();
    for q in queries {
        let mut so = search_opts(env, remote.clone(), Some(want));
        let mut res = resolver::resolve(&cat.apps, q, &so);
        if res.status == ResolutionStatus::NotFound && !explicit {
            // The preferred scope has nothing: fall back to wherever the remote lives, and say so.
            so.scope = None;
            res = resolver::resolve(&cat.apps, q, &so);
        }
        let idx = match res.status {
            ResolutionStatus::NotFound => {
                problems.push(not_found(q, &remote));
                continue;
            }
            _ => match choose(env, &cat.apps, &res, st, opts.yes) {
                Ok(i) => i,
                Err(e) => {
                    problems.push(e);
                    continue;
                }
            },
        };
        let app = cat.apps[idx].clone();
        if !chosen.iter().any(|c| c.identity() == app.identity()) {
            chosen.push(app);
        }
    }
    if !problems.is_empty() {
        return Err(combine(problems));
    }

    // Flatpak would skip these anyway; say so instead of reporting a fresh install.
    let have = env.backend.installed()?;
    let (already, chosen): (Vec<App>, Vec<App>) = chosen.into_iter().partition(|a| {
        have.iter()
            .any(|i| i.id == a.id && !i.is_runtime && i.installation == a.scope.as_str())
    });
    if !already.is_empty() {
        let names: Vec<String> = already.iter().map(service::label).collect();
        env.io
            .out(&format!("Already installed: {}\n", names.join(", ")));
    }
    if chosen.is_empty() {
        return Ok(());
    }

    let plan: Vec<PlanLine> = chosen
        .iter()
        .map(|a| PlanLine {
            app: a,
            scope_note: (a.scope != want && !explicit).then(|| {
                format!(
                    "remote '{}' is not configured for the {want} installation",
                    a.remote
                )
            }),
        })
        .collect();
    env.io.out(&output::install_plan(&plan, st));
    let q = if chosen.len() == 1 {
        format!("Install {}?", service::label(&chosen[0]))
    } else {
        format!("Install {} applications?", chosen.len())
    };
    confirm(
        env,
        &q,
        true,
        opts.yes || !env.config.general.confirm_install,
    )?;
    let rep = service::install(env.backend, &chosen);
    report(env, "Installed", "install", &rep)
}

fn remove(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let installed: Vec<Installed> = env
        .backend
        .installed()?
        .into_iter()
        .filter(|i| opts.all || !i.is_runtime)
        .collect();
    let pool: Vec<App> = installed.iter().map(Installed::to_app).collect();
    let so = search_opts(env, opts.remote.clone(), opts.scope);
    let mut chosen: Vec<usize> = Vec::new();
    let mut problems = Vec::new();
    for q in queries {
        let res = resolver::resolve(&pool, q, &so);
        let idx = match res.status {
            ResolutionStatus::NotFound => {
                problems.push(Error::NotFound(format!(
                    "No installed application matches '{q}'.\n\nSee what is installed:\n  ifhf list"
                )));
                continue;
            }
            _ => match choose(env, &pool, &res, st, opts.yes) {
                Ok(i) => i,
                Err(e) => {
                    problems.push(e);
                    continue;
                }
            },
        };
        if !chosen.contains(&idx) {
            chosen.push(idx);
        }
    }
    if !problems.is_empty() {
        return Err(combine(problems));
    }
    let items: Vec<Installed> = chosen.iter().map(|&i| installed[i].clone()).collect();
    let mut plan = String::from("The following will be removed:\n\n");
    for i in &items {
        plan.push_str(&format!(
            "  {} ({}) [{}]\n",
            st.bold(&i.name),
            i.id,
            i.installation
        ));
    }
    plan.push('\n');
    if opts.delete_data {
        plan.push_str(&st.red("WARNING: --delete-data permanently deletes these applications' user data (settings, saved files).\n\n"));
    } else {
        plan.push_str("Application data is kept; reinstalling restores it.\n\n");
    }
    env.io.out(&plan);
    let q = if items.len() == 1 {
        format!("Remove {}?", items[0].name)
    } else {
        format!("Remove {} applications?", items.len())
    };
    confirm(
        env,
        &q,
        !opts.delete_data,
        opts.yes || (!env.config.general.confirm_remove && !opts.delete_data),
    )?;
    let rep = service::remove(env.backend, &items, opts.delete_data);
    report(env, "Removed", "remove", &rep)
}

fn search(env: &mut Env, opts: &Opts, query: &str) -> Result<()> {
    let st = style(env, opts);
    let cat = load_catalog(env)?;
    let so = search_opts(env, effective_remote(env, opts), opts.scope);
    let cands = resolver::search(&cat.apps, query, &so);
    if cands.is_empty() {
        return Err(Error::NotFound(format!(
            "No applications found matching '{query}'."
        )));
    }
    let limit = match opts.limit {
        Some(0) => usize::MAX,
        Some(n) => n,
        None => DEFAULT_LIMIT,
    };
    let shown = &cands[..cands.len().min(limit)];
    env.io.out(&output::search_results(&cat.apps, shown, st));
    if shown.len() < cands.len() {
        env.io.out(&format!(
            "Showing {} of {} results. Use --limit 0 to show all.\n",
            shown.len(),
            cands.len()
        ));
    }
    Ok(())
}

fn info(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let cat = load_catalog(env)?;
    let installed = env.backend.installed()?;
    let remote = effective_remote(env, opts);
    let so = search_opts(env, remote.clone(), opts.scope);
    let installed_pool: Vec<App> = installed.iter().map(Installed::to_app).collect();
    let mut problems = Vec::new();
    let mut blocks = Vec::new();
    for q in queries {
        let mut res = resolver::resolve(&cat.apps, q, &so);
        let mut from_installed = false;
        if res.status == ResolutionStatus::NotFound {
            // Installed but no longer in any catalogue: show what we know.
            res = resolver::resolve(&installed_pool, q, &so);
            from_installed = true;
        }
        let pool = if from_installed {
            &installed_pool
        } else {
            &cat.apps
        };
        let idx = match res.status {
            ResolutionStatus::NotFound => {
                problems.push(not_found(q, &remote));
                continue;
            }
            _ => match choose(env, pool, &res, st, false) {
                Ok(i) => i,
                Err(e) => {
                    problems.push(e);
                    continue;
                }
            },
        };
        let app = &pool[idx];
        let here: Vec<&Installed> = installed
            .iter()
            .filter(|i| i.id == app.id && !i.is_runtime)
            .collect();
        blocks.push(output::info(app, &here, st));
    }
    env.io.out(&blocks.join("\n"));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(combine(problems))
    }
}

fn list(env: &mut Env, opts: &Opts) -> Result<()> {
    let st = style(env, opts);
    let filter = opts.filter.as_deref().map(str::to_lowercase);
    let mut rows: Vec<&Installed> = Vec::new();
    let all = env.backend.installed()?;
    for i in &all {
        if (!opts.all && i.is_runtime) || opts.scope.is_some_and(|s| i.scope() != s) {
            continue;
        }
        if opts.remote.as_deref().is_some_and(|r| i.origin != r) {
            continue;
        }
        if let Some(f) = &filter
            && !i.id.to_lowercase().contains(f)
            && !i.name.to_lowercase().contains(f)
        {
            continue;
        }
        rows.push(i);
    }
    rows.sort_by_key(|i| (i.name.to_lowercase(), i.id.clone(), i.installation.clone()));
    if rows.is_empty() {
        env.io.err("No matching installed applications.\n");
        return Ok(());
    }
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|i| {
            vec![
                i.name.clone(),
                i.id.clone(),
                i.branch.clone(),
                i.installation.clone(),
                i.origin.clone(),
            ]
        })
        .collect();
    let pretty = env.io.stdout_is_tty();
    env.io.out(&output::table(
        &["Name", "Application ID", "Branch", "Scope", "Origin"],
        &table,
        pretty,
        st,
    ));
    Ok(())
}

fn remotes(env: &mut Env) -> Result<()> {
    let st = Style {
        color: env.config.general.color && env.io.color_allowed(),
    };
    let rows: Vec<Vec<String>> = env
        .backend
        .remotes()?
        .into_iter()
        .map(|r| {
            vec![
                r.name,
                r.scope.to_string(),
                if r.enabled { "enabled" } else { "disabled" }.into(),
                r.url,
            ]
        })
        .collect();
    let pretty = env.io.stdout_is_tty();
    env.io.out(&output::table(
        &["Remote", "Scope", "Status", "URL"],
        &rows,
        pretty,
        st,
    ));
    Ok(())
}

fn show_updates(env: &mut Env, ups: &[&Update]) {
    let st = Style {
        color: env.config.general.color && env.io.color_allowed(),
    };
    let rows: Vec<Vec<String>> = ups
        .iter()
        .map(|u| {
            vec![
                u.name.clone(),
                u.id.clone(),
                u.branch.clone(),
                u.installation.clone(),
                u.origin.clone(),
                if u.is_runtime { "runtime" } else { "app" }.into(),
            ]
        })
        .collect();
    let pretty = env.io.stdout_is_tty();
    env.io.out(&output::table(
        &[
            "Name",
            "Application ID",
            "Branch",
            "Scope",
            "Origin",
            "Type",
        ],
        &rows,
        pretty,
        st,
    ));
}

fn updates(env: &mut Env) -> Result<()> {
    let ups = env.backend.updates()?;
    if ups.is_empty() {
        env.io.out("Everything is up to date.\n");
        return Ok(());
    }
    show_updates(env, &ups.iter().collect::<Vec<_>>());
    Ok(())
}

fn update(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let ups = env.backend.updates()?;
    let mut selected: Vec<&Update> = ups.iter().collect();
    let mut up_to_date: Vec<String> = Vec::new();
    if !queries.is_empty() {
        let installed: Vec<Installed> = env
            .backend
            .installed()?
            .into_iter()
            .filter(|i| !i.is_runtime)
            .collect();
        let pool: Vec<App> = installed.iter().map(Installed::to_app).collect();
        let so = search_opts(env, None, opts.scope);
        let mut ids: Vec<(String, String)> = Vec::new(); // (id, installation)
        let mut problems = Vec::new();
        for q in queries {
            let res = resolver::resolve(&pool, q, &so);
            let idx = match res.status {
                ResolutionStatus::NotFound => {
                    problems.push(Error::NotFound(format!("No installed application matches '{q}'.\n\nSee what is installed:\n  ifhf list")));
                    continue;
                }
                _ => match choose(env, &pool, &res, st, opts.yes) {
                    Ok(i) => i,
                    Err(e) => {
                        problems.push(e);
                        continue;
                    }
                },
            };
            ids.push((
                installed[idx].id.clone(),
                installed[idx].installation.clone(),
            ));
            if !ups
                .iter()
                .any(|u| u.id == installed[idx].id && u.installation == installed[idx].installation)
            {
                up_to_date.push(installed[idx].name.clone());
            }
        }
        if !problems.is_empty() {
            return Err(combine(problems));
        }
        selected.retain(|u| {
            ids.iter()
                .any(|(id, inst)| *id == u.id && *inst == u.installation)
        });
    }
    if !up_to_date.is_empty() {
        env.io
            .out(&format!("Already up to date: {}\n", up_to_date.join(", ")));
    }
    if selected.is_empty() {
        if up_to_date.is_empty() {
            env.io.out("Everything is up to date.\n");
        }
        return Ok(());
    }
    env.io
        .out(&format!("{} update(s) available:\n\n", selected.len()));
    show_updates(env, &selected);
    env.io.out("\n");
    confirm(env, "Apply these updates?", true, opts.yes)?;
    let mut by_inst: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();
    for u in &selected {
        by_inst
            .entry(u.installation.clone())
            .or_default()
            .push((u.id.clone(), u.name.clone()));
    }
    // A full update (no names) lets Flatpak handle the whole installation, runtimes included.
    let targets: Vec<(String, Vec<(String, String)>)> = by_inst
        .into_iter()
        .map(|(inst, apps)| {
            if queries.is_empty() {
                (inst, vec![])
            } else {
                (inst, apps)
            }
        })
        .collect();
    let rep = service::update(env.backend, &targets);
    report(env, "Updated", "update", &rep)
}

fn sync(env: &mut Env) -> Result<()> {
    env.io.out("Refreshing metadata...\n");
    env.backend.sync()?;
    let cat = metadata::refresh(env.backend, env.config, env.cache_dir.as_deref())?;
    for n in &cat.notes {
        env.io.err(&format!("Note: {n}\n"));
    }
    env.io.out(&format!(
        "Metadata synchronised: {} applications indexed.\n",
        cat.apps.len()
    ));
    Ok(())
}
