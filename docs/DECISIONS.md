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

## Known gaps (not implemented)

- Interactive TUI (`ifhf` with no arguments prints help).
- Native libflatpak backend (not justified by measurements).
- LICENSE file (the project owner has not chosen one).
- `flatpak search` ranks differently from IFHF; they are not expected to return identical order.

## Measured performance trade-offs

From `bench/RESULTS.md` (i5-10310U, `powersave` governor, noisy: σ of 5–15 ms on fast commands):

- **Large wins (no Flatpak process involved):** search ~200× faster, details-by-ID ~25×, startup ~15×,
  memory ~24× lower (20 MB vs 450 MB for a search).
- **Parity within noise:** `list`, `remotes`, `updates`, `install` (the work is Flatpak's own).
- **Slower by design:** `remove` (~+60–85 ms), `info` on an installed app (~+40 ms), `updates` (~+0.1 s)
  and `list` (+0–15 ms). Reasons: `remove` and `install` ask Flatpak what is installed first (to resolve
  names, report "already installed" and report partial failures exactly); `info` also loads the
  index; `updates` queries the user and system installations separately. These are the price of
  name resolution and safer reporting. They could be cut by reading installed state directly from
  Flatpak's on-disk layout, which was rejected as relying on undocumented internals.
- **Cached search vs the 20 ms target:** 12–35 ms depending on the query and CPU state; the target is
  met for some queries, not reliably on this machine. Index parsing and the resolver scan run on
  several threads for large catalogues (identical results to sequential, covered by tests).
