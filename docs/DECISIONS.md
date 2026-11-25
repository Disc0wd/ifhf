# Design decisions and deviations from the specification

Recorded as required by spec section 18. Anything not listed here follows `IFHF_Specification.md`.

## Scope of what was built

The specification asks coding agents to start with Phases 0–1. The request for this build was
"fully finished and tested", so Phases 0–5 plus the user-facing parts of Phase 6 (README, tests,
versioning) are implemented. Interactive TUI mode (spec 3.3, explicitly not required) is not.

## Decisions

| # | Decision | Reason |
|:--|:--|:--|
| 1 | Hand-written argument parser, no `clap` | Pacman clusters (`-Syu`) do not fit a derive parser; fewer dependencies; faster startup. Both syntaxes parse to one `Command` enum and share all logic. |
| 2 | Dependencies: `quick-xml`, `flate2`, `toml`, `serde` only | No `anyhow`/`thiserror`/`clap`: a small local `Error` enum maps to exit codes. `flate2` is needed because OCI remotes (Fedora) ship only `appstream.xml.gz`. |
| 3 | **Text index instead of SQLite** | Spec 6.2/18: add persistence only if measured. Parsing the 52 MB flathub XML costs ~0.5 s per command; a plain tab-separated index loads in a few ms with no dependency, is atomic (temp file + rename), disposable and rebuildable. SQLite/FTS5 was not evaluated: the plain index plus an in-memory scan already searches ~3,500 apps in about 10–30 ms (see `bench/RESULTS.md`), so there is no measured need. |
| 4 | Subprocess backend only | Spec 7.3: no native libflatpak without evidence. Measured: for `list`/`remotes` the cost is dominated by Flatpak's own process; the large wins are in search/resolution/details, which need no Flatpak process at all. |
| 5 | Scope fallback | Spec 4.1 says default scope is user. On machines where the remote exists only system-wide, `flatpak install --user flathub …` fails. When the scope was *not* given explicitly and the preferred scope has no match, IFHF retries across scopes and states the scope and why in the plan. An explicit `--user`/`--system` is strict. |
| 6 | `--yes` refuses fuzzy-only matches | Spec 4.1 FR-006: "never silently select an ambiguous match". A unique but weak match (below a normalised exact name) is shown for confirmation interactively, but under `--yes` it requires an explicit choice or a precise ID. Found while testing: `install terminal` would otherwise pick `terminalpolyominos`. |
| 7 | Installed check before install | Flatpak reports "already installed" as success; IFHF says "Already installed: X" instead of "Installed: X". |
| 8 | Partial failure is verified, not inferred | After a failed transaction IFHF asks Flatpak what is installed to decide per application. |
| 9 | `confirm_remove = false` cannot skip the `--delete-data` confirmation | Destructive and irreversible; spec 4.2 says do not make it imply ordinary removal deletes data. |
| 10 | AppStream `type="desktop"` accepted | It is the legacy alias of `desktop-application` and is how Fedora's metadata labels every app (found against real data). |
| 11 | `uninstall`, not `remove`, is sent to Flatpak | `flatpak uninstall` is the documented command. |
| 12 | Updates ask each installation once, no `flatpak remotes` pre-check | `flatpak remote-ls --updates --user` returns an empty list (exit 0) when there are no user remotes, so the pre-check cost a wasted process launch. |
| 13 | Named extra system installations are not read | `FlatpakCli` reads the user dir and `FLATPAK_SYSTEM_DIR`/`/var/lib/flatpak`. Supporting more needs `flatpak --installations` plus `--installation=NAME` plumbing; not needed for the target machine. |
| 14 | Staleness note after 7 days | FR-028: users are told when metadata may be out of date (printed to stderr, never alters results). |
| 15 | Metadata is read from Flatpak's on-disk AppStream files | Spec 6.2 asks for "supported interfaces or documented locations"; there is no CLI that dumps AppStream data efficiently (`flatpak search` is the slow path being replaced). The path layout is stable across Flatpak 1.x; if a file is missing IFHF reports it and points to `ifhf sync` instead of guessing. |
| 16 | Terminal styling is layered: `color` ⊂ `fancy` (layout, symbols) ⊂ is-a-terminal | Scripts must keep parsing the same bytes. Colour never changes the text (tested by stripping escape codes), and ASCII mode is verified to be pure ASCII. |
| 17 | Choice lists show only candidates within the ambiguity margin of the best match | Found while reviewing real output: `install firefox` listed 10 entries, 7 of them weak description matches. Those are noise, not choices. |
| 18 | The index header carries a parser revision (`r<N>`) as well as the version | A parser fix (e.g. de-duplicating the developer name) must invalidate indexes written by the old parser, not only releases. |
| 19 | A user "no" prints a dim `Cancelled.` and exits 6 | Declining is not a failure; no alarm colours. |

## Known gaps (not implemented)

- Interactive TUI (`ifhf` with no arguments prints help).
- Native libflatpak backend (not justified by measurements).
- LICENSE file (the project owner has not chosen one).
- `flatpak search` ranks differently from IFHF; they are not expected to return identical order.

## Measured performance trade-offs

From `bench/RESULTS.md` (i5-10310U, `powersave` governor; noisy, σ of 1–7 ms on fast commands). Both
tools write to a pipe that hyperfine drains, so neither gets a free `/dev/null`.

- **Large wins (no Flatpak process involved):** search ~210× faster, details-by-ID ~30×, startup ~17×,
  memory ~23× lower (19 MiB vs 439 MiB for a search).
- **Parity within noise:** `list`, `updates`, `install`, `remove` (remove is +15 ms).
- **Still slower:** `remotes` (+7 ms: process spawn and output parsing around the same Flatpak
  call) and `info` on an installed app (+24 ms: it loads the index and asks Flatpak what is
  installed, which `flatpak info` does not need to do).

### What was done about it

An earlier run had `list` +10 ms, `remove` +84 ms, `updates` +120 ms and installed `info` +43 ms. Causes and fixes:

| Cause | Fix |
|:---|:---|
| `list` asked Flatpak for runtimes and filtered them out (+10 ms inside Flatpak) | `installed(include_runtimes)`: apps only unless `--all` |
| `updates` queried the user and system installations one after the other | Both queried concurrently; result order stays user, then system |
| `install`/`info` loaded the index, then asked Flatpak what is installed | Both run at once (`std::thread::scope`) |
| `update <app>` waited for the network check, then for the installed lookup | The lookup rides along with the check |
| Benchmark gave `flatpak` a `/dev/null` output but made ifhf read a pipe | `--output pipe` for both |

### Not done, on purpose

Reading installed apps straight from Flatpak's on-disk layout (`<installation>/app/<id>/…`) would
remove the remaining `flatpak list` launch and make `info`/`remove`/`list` a few milliseconds
faster. It depends on Flatpak's internal directory format, which is not a documented API, so it
was left out; the remaining gap is 7–25 ms. If it is ever added it must fall back to the CLI and be
differential-tested against `flatpak list`.

### Cached search vs the 20 ms target

10–26 ms depending on the query and CPU state, so the target is met for most queries, not all. Index
parsing and the resolver scan run on several threads for large catalogues (results identical to
sequential, covered by tests).
