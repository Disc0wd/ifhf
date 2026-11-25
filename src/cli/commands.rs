use super::Env;
use super::args::{Command, HELP, Invocation, Opts};
use crate::core::models::{App, Installed, Update};
use crate::core::resolver::{
    self, AMBIGUITY_MARGIN, Candidate, MatchKind, ResolutionResult, ResolutionStatus, SearchOpts,
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
            let st = out_style(env, opts.no_color);
            env.io.out(&output::help(HELP, st));
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
        Command::Updates => updates(env, &opts),
        Command::Update(q) => update(env, &opts, &q),
        Command::Sync => sync(env),
        Command::Remotes => remotes(env, &opts),
    }
}

fn style(env: &Env, opts: &Opts) -> Style {
    out_style(env, opts.no_color)
}

/// Fancy layout on a terminal; colour only if config, `--no-color` and `NO_COLOR` allow it.
fn out_style(env: &Env, no_color: bool) -> Style {
    let fancy = env.io.stdout_is_tty();
    Style {
        fancy,
        color: fancy && env.config.general.color && !no_color && env.io.color_allowed(),
        unicode: env.io.unicode(),
    }
}

fn err_style(env: &Env) -> Style {
    Style {
        fancy: env.io.stderr_is_tty(),
        color: env.io.stderr_is_tty() && env.config.general.color && env.io.stderr_color_allowed(),
        unicode: env.io.unicode(),
    }
}

fn notes(env: &mut Env, list: &[String]) {
    let st = err_style(env);
    for n in list {
        env.io.err(&output::note(st, n));
    }
}

fn ok_line(st: Style, text: &str) -> String {
    if st.fancy {
        format!("{} {text}\n", st.ok_mark())
    } else {
        format!("{text}\n")
    }
}

fn load_catalog(env: &mut Env) -> Result<Catalog> {
    let cat = metadata::load(env.backend, env.config, env.cache_dir.as_deref())?;
    notes(env, &cat.notes);
    Ok(cat)
}

/// Load the catalogue while Flatpak lists the installed apps. Both are mostly waiting (on a file
/// read and on a subprocess), so overlapping them hides the shorter one.
fn catalog_and_installed(env: &mut Env) -> (Result<Catalog>, Result<Vec<Installed>>) {
    let backend = env.backend;
    std::thread::scope(|s| {
        let lookup = s.spawn(move || backend.installed(false));
        let cat = load_catalog(env);
        let have = lookup
            .join()
            .unwrap_or_else(|_| Err(Error::Backend("installed-apps lookup panicked".into())));
        (cat, have)
    })
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
fn confirm(env: &mut Env, st: Style, question: &str, default_yes: bool, skip: bool) -> Result<()> {
    if skip {
        return Ok(());
    }
    if !env.io.interactive() {
        return Err(Error::NeedsConfirmation(
            "Confirmation is required but input is not a terminal.\n\nRe-run with --yes to proceed without asking.".into(),
        ));
    }
    match env
        .io
        .ask(&output::prompt(st, question, default_yes))
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
    // Only the genuine rivals are offered; weaker matches further down are noise, not choices.
    let top = res.candidates[0].score;
    let rivals: Vec<Candidate> = res
        .candidates
        .iter()
        .copied()
        .take_while(|c| c.score + AMBIGUITY_MARGIN > top)
        .collect();
    let shown: &[Candidate] = &rivals[..rivals.len().min(MAX_CHOICES)];
    let list = output::candidate_list(apps, shown, st);
    let hidden = rivals.len() - shown.len();
    let more = if hidden > 0 {
        format!("({hidden} more not shown; be more specific.)\n")
    } else {
        String::new()
    };
    let head = if rivals.len() > 1 {
        format!("Multiple applications match '{}':", res.query)
    } else {
        format!("'{}' is not an exact match. Closest result:", res.query)
    };
    if !env.io.interactive() {
        return Err(Error::Ambiguous(format!(
            "{head}\n\n{list}{more}Specify an application ID, or narrow it with --remote / --user / --system."
        )));
    }
    env.io.out(&format!("{}\n\n{list}{more}", st.bold(&head)));
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

fn report(env: &mut Env, st: Style, verb: &str, action: &str, rep: &OpReport) -> Result<()> {
    if !rep.done.is_empty() {
        env.io.out(&output::done_lines(st, verb, &rep.done));
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
    let (cat, have) = catalog_and_installed(env);
    let cat = cat?;
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
    let have = have?;
    let (already, chosen): (Vec<App>, Vec<App>) = chosen.into_iter().partition(|a| {
        have.iter()
            .any(|i| i.id == a.id && !i.is_runtime && i.installation == a.scope.as_str())
    });
    if !already.is_empty() {
        let names: Vec<String> = already.iter().map(service::label).collect();
        env.io.out(&format!(
            "{}\n",
            st.dim(&format!("Already installed: {}", names.join(", ")))
        ));
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
        st,
        &q,
        true,
        opts.yes || !env.config.general.confirm_install,
    )?;
    let rep = service::install(env.backend, &chosen);
    report(env, st, "Installed", "install", &rep)
}

fn remove(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let installed: Vec<Installed> = env
        .backend
        .installed(opts.all)?
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
    let plan = output::remove_plan(&items, opts.delete_data, st);
    env.io.out(&plan);
    let q = if items.len() == 1 {
        format!("Remove {}?", items[0].name)
    } else {
        format!("Remove {} applications?", items.len())
    };
    confirm(
        env,
        st,
        &q,
        !opts.delete_data,
        opts.yes || (!env.config.general.confirm_remove && !opts.delete_data),
    )?;
    let rep = service::remove(env.backend, &items, opts.delete_data);
    report(env, st, "Removed", "remove", &rep)
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
            "{}\n",
            st.dim(&format!(
                "Showing {} of {} results. Use --limit 0 to show all.",
                shown.len(),
                cands.len()
            ))
        ));
    }
    Ok(())
}

fn info(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    let (cat, installed) = catalog_and_installed(env);
    let cat = cat?;
    let installed = installed?;
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
    let all = env.backend.installed(opts.all)?;
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
        &|col, c| match col {
            0 => st.bold(c),
            1 => st.id(c),
            _ => st.dim(c),
        },
    ));
    Ok(())
}

fn remotes(env: &mut Env, opts: &Opts) -> Result<()> {
    let st = style(env, opts);
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
        &|col, c| match (col, c) {
            (0, _) => st.bold(c),
            (2, "enabled") => st.green(c),
            (2, _) => st.red(c),
            _ => st.dim(c),
        },
    ));
    Ok(())
}

fn show_updates(env: &mut Env, st: Style, ups: &[&Update]) {
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
        &|col, c| match col {
            0 => st.bold(c),
            1 => st.id(c),
            _ => st.dim(c),
        },
    ));
}

fn updates(env: &mut Env, opts: &Opts) -> Result<()> {
    let st = style(env, opts);
    let ups = env.backend.updates()?;
    if ups.is_empty() {
        env.io.out(&ok_line(st, "Everything is up to date."));
        return Ok(());
    }
    show_updates(env, st, &ups.iter().collect::<Vec<_>>());
    Ok(())
}

fn update(env: &mut Env, opts: &Opts, queries: &[String]) -> Result<()> {
    let st = style(env, opts);
    // The update check is a network round trip; the installed-apps lookup rides along with it.
    let backend = env.backend;
    let want_installed = !queries.is_empty();
    let (ups, installed_now) = std::thread::scope(|s| {
        let lookup = want_installed.then(|| s.spawn(move || backend.installed(false)));
        let ups = backend.updates();
        let installed_now = lookup.map(|h| {
            h.join()
                .unwrap_or_else(|_| Err(Error::Backend("installed-apps lookup panicked".into())))
        });
        (ups, installed_now)
    });
    let ups = ups?;
    let mut selected: Vec<&Update> = ups.iter().collect();
    let mut up_to_date: Vec<String> = Vec::new();
    if !queries.is_empty() {
        let installed: Vec<Installed> = installed_now
            .expect("requested whenever queries are present")?
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
            env.io.out(&ok_line(st, "Everything is up to date."));
        }
        return Ok(());
    }
    env.io.out(&format!(
        "{}\n\n",
        st.bold(&format!("{} update(s) available:", selected.len()))
    ));
    show_updates(env, st, &selected);
    env.io.out("\n");
    confirm(env, st, "Apply these updates?", true, opts.yes)?;
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
    report(env, st, "Updated", "update", &rep)
}

fn sync(env: &mut Env) -> Result<()> {
    let st = out_style(env, false);
    if st.fancy {
        env.io
            .out(&format!("{} Refreshing metadata…\n", st.step_mark()));
    } else {
        env.io.out("Refreshing metadata...\n");
    }
    env.backend.sync()?;
    let cat = metadata::refresh(env.backend, env.config, env.cache_dir.as_deref())?;
    notes(env, &cat.notes);
    env.io.out(&ok_line(
        st,
        &format!(
            "Metadata synchronised: {} applications indexed.",
            cat.apps.len()
        ),
    ));
    Ok(())
}
