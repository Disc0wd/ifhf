# IFHF

A friendly command-line frontend for [Flatpak](https://flatpak.org). Install, remove and search
applications by ordinary name instead of memorising application IDs and remote names.

```console
$ ifhf install spotify
Spotify
Online music streaming service

Application: com.spotify.Client
Remote:      flathub
Scope:       user

Install Spotify? [Y/n]
```

IFHF is a thin layer. **Flatpak still downloads, verifies, resolves dependencies and deploys.**
IFHF never touches OSTree, never bypasses signature checks or permissions, runs no daemon, and
needs no hosted service.

## Install

```bash
cargo build --release
install -Dm755 target/release/ifhf ~/.local/bin/ifhf
```

Requires Rust (edition 2024, tested with 1.100) and a working `flatpak` on `PATH`.

## Usage

Two equivalent syntaxes. Both parse into one internal command, so they always behave the same.

| Natural | Pacman-style | What it does |
|:---|:---|:---|
| `ifhf install spotify discord` | `ifhf -S spotify discord` | Install by name or ID (one confirmation) |
| `ifhf remove spotify` | `ifhf -R spotify` | Remove; **user data is kept** |
| `ifhf search browser` | `ifhf -Ss browser` | Search every enabled remote |
| `ifhf info firefox` | `ifhf -Si firefox` | Details and installed status |
| `ifhf list` | `ifhf -Q` | Installed applications |
| `ifhf list --search fire` | `ifhf -Qs fire` | Filter installed applications |
| `ifhf updates` | `ifhf -Qu` | List available updates |
| `ifhf update [app...]` | `ifhf -Syu` | Apply updates |
| `ifhf sync` | `ifhf -Sy` | Refresh metadata and rebuild the index |
| `ifhf remotes` | | Configured remotes |

Options (either position): `--remote <name>`, `--user` / `--system`, `--yes` (`--noconfirm`),
`--delete-data` (remove only), `--all` (include runtimes), `--limit <n>` (search; `0` = all),
`--no-color`. Run `ifhf --help` for the full text.

### Safety rules

- An exact application ID always wins over fuzzy name matches.
- **Ambiguity is never resolved silently.** When several applications are similarly relevant
  (for example the same name in two remotes) you are asked to choose; in scripts the command fails
  with exit status 4 and lists the candidates.
- `--yes` never accepts a fuzzy-only match (anything weaker than an exact/normalised name).
- Without a terminal on stdin, IFHF never waits: confirmation needs `--yes` (exit status 5).
- Removing keeps application data. `--delete-data` shows a warning and defaults to **No**, and
  `confirm_remove = false` cannot skip that confirmation.
- Every ID and remote name is validated before it reaches a Flatpak command line, and arguments are
  passed as separate argv entries — never through a shell.

### Scope

The default scope is `user` (configurable). If the remote that provides an application is only
configured system-wide (common on Fedora), IFHF falls back to `system` and says so in the plan.
An explicit `--user`/`--system` is never overridden.

### Exit status

| Code | Meaning |
|---:|:---|
| 0 | Success |
| 1 | Failure (Flatpak error, metadata problem, partial failure) |
| 2 | Usage error |
| 3 | Not found |
| 4 | Ambiguous match |
| 5 | Confirmation required (non-interactive without `--yes`) |
| 6 | Cancelled by the user |

When several applications are requested and some fail, IFHF reports exactly which succeeded and
which failed (it asks Flatpak what is installed afterwards rather than guessing). It does not claim
atomicity.

## Configuration

`$XDG_CONFIG_HOME/ifhf/config.toml` (default `~/.config/ifhf/config.toml`). Everything is optional.
Precedence: command-line option, then config, then built-in default. Unknown keys are ignored;
invalid values are reported with the file and line.

```toml
[general]
default_scope = "user"      # or "system"
default_remote = ""         # restrict to one remote unless --remote is given
confirm_install = true
confirm_remove = true
color = true                # NO_COLOR and non-terminals always disable colour

[search]
include_descriptions = true
fuzzy_matching = true       # substring and ID-fragment matches

[cache]
enabled = true
max_age_hours = 24          # 0 = always rebuild
```

## How it works

```
cli ──► core (resolver, service) ──► backend (trait) ──► flatpak executable
 │            │
 │            └────────────► metadata (AppStream parser, index)
 └─► output (pure formatting)        config
```

- **Metadata** comes from the AppStream files Flatpak already keeps for each remote
  (`<installation>/appstream/<remote>/<arch>/active/appstream.xml[.gz]`). Nothing is downloaded by
  IFHF; `ifhf sync` asks Flatpak to refresh them.
- **Index.** Parsing ~50 MB of XML takes ~0.5 s, so the parsed catalogue is cached in
  `$XDG_CACHE_HOME/ifhf/index.tsv` (a plain text file; delete it any time). It is replaced
  atomically, so a failed refresh keeps the last good copy. It is rebuilt when a source file, a
  remote, or the installation changes, or when it is older than `max_age_hours`. A warm search never
  starts a subprocess. A failed or unreadable refresh is an error, never an empty catalogue.
- **Resolver** (`src/core/resolver.rs`) is pure and deterministic: exact ID 1000, exact name 900,
  normalised name 850, name prefix 700, whole words 600, substring 450, ID fragment 400, keyword
  300, summary 200, description 100. Two different candidates within 100 points are "ambiguous".
  Ties break on name, ID, remote.
- **Backend** (`src/backend`) is the only code that runs Flatpak. `MockBackend` lets the whole CLI
  be tested without touching a real installation.

## Development

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

| Suite | What it covers |
|:---|:---|
| unit (`src/**`) | arg parsing & equivalence, resolver ranking/ambiguity, config, AppStream parsing, index round-trip/corruption/freshness, argv construction |
| `tests/cli.rs` | every command end to end against `MockBackend` and fixture metadata (confirmations, ambiguity, scope, partial failures, cache invalidation) |
| `tests/backend.rs` | the real `FlatpakCli` against a stub `flatpak` script (parsing, argv, failures, missing executable, hostile input) |
| `tests/bin.rs` | the compiled binary with a stub flatpak and isolated XDG/Flatpak directories (exit codes, config, non-interactive behaviour) |

Tests never touch the real Flatpak installation. `IFHF_FLATPAK=/path/to/flatpak` overrides the
executable.

## Benchmarks

```bash
cargo build --release && bench/run.sh      # needs hyperfine; writes bench/RESULTS.md
```

Headline results (i5-10310U laptop, noisy; full tables and method in
[`bench/RESULTS.md`](bench/RESULTS.md)):

| Task | `flatpak` | `ifhf` |
|:---|---:|---:|
| Search `firefox` | 2.69 s | 13 ms |
| Details by exact ID | 1.56 s | 58 ms |
| Startup | 32 ms | 2 ms |
| Peak memory (search) | 439 MiB | 19 MiB |
| List installed / remotes / updates / install | baseline | about equal |
| Remove / installed-app `info` | baseline | 40–85 ms **slower** (extra lookups) |
| First search after index deleted | n/a | 0.58 s |

The wins come from searching a local index instead of Flatpak's search; where IFHF only wraps a
Flatpak operation it adds a little overhead. See [`docs/DECISIONS.md`](docs/DECISIONS.md) for design
decisions, measured trade-offs and deviations from the specification.

## Limitations

- Only the default system installation and the user installation are read (named extra system
  installations are not).
- Only the host architecture is indexed.
- Apps whose remote ships no AppStream metadata cannot be found by name (exact IDs still need
  metadata to resolve).
- No interactive TUI yet (spec: not required for the first release).
- No LICENSE file has been chosen yet.
