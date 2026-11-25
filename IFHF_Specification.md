# IFHF --- Product & Technical Specification

**Project:** IFHF\
**Executable:** `ifhf`\
**Parent project family:** IFHX\
**Status:** Draft specification\
**Primary platform:** Linux\
**Implementation language:** Rust\
**Backend:** Flatpak\
**Infrastructure requirement:** No hosted services, databases, APIs, or
paid infrastructure

------------------------------------------------------------------------

## 1. Product overview

IFHF is a lightweight, user-friendly command-line frontend for managing
Flatpak applications.

Its purpose is to make common Flatpak tasks easier to discover and
perform without requiring users to memorise application IDs, remote
names, or long command sequences.

IFHF should combine:

-   The convenience and familiar command conventions of package managers
    such as pacman.
-   Human-readable application discovery and resolution.
-   Sensible defaults and clear confirmation prompts.
-   Efficient local metadata lookup.
-   Reliable use of Flatpak's existing installation and deployment
    mechanisms.

IFHF is **not** a replacement for Flatpak, OSTree, or the Flatpak
security model. Flatpak remains responsible for downloading, verifying,
resolving dependencies, and deploying applications.

### 1.1 Core product principle

> Make the common operation obvious. Keep advanced control available. Do
> not make users understand the implementation to use the product.

### 1.2 Goals

1.  Make common Flatpak operations shorter and easier to remember.
2.  Allow users to search for applications by ordinary names and
    descriptions.
3.  Resolve human-readable names to application IDs safely.
4.  Handle multiple configured remotes transparently.
5.  Provide useful, understandable output and errors.
6.  Keep common local operations fast.
7.  Remain maintainable without operating any infrastructure.

### 1.3 Non-goals

IFHF will not:

-   Replace Flatpak or OSTree.
-   Host an application catalogue or package repository.
-   Operate a central database or API.
-   Implement its own package download protocol.
-   Bypass Flatpak signature verification or permission checks.
-   Install or remove applications through direct manipulation of OSTree
    data.
-   Provide a graphical software store in its initial release.
-   Run a permanent background daemon.
-   Attempt to improve the runtime performance of installed
    applications.

------------------------------------------------------------------------

## 2. Target users and use cases

### 2.1 Target users

-   Linux desktop users who install applications through Flatpak.
-   Users who prefer a terminal but do not want to memorise Flatpak
    application IDs.
-   Users with multiple Flatpak remotes.
-   Experienced users who want compact commands and predictable output.
-   Less experienced users who benefit from descriptive prompts and
    sensible defaults.

### 2.2 Primary use cases

  -----------------------------------------------------------------------------------------
  ID                      User goal               Example
  ----------------------- ----------------------- -----------------------------------------
  UC-01                   Install an application  `ifhf install spotify`
                          by name                 

  UC-02                   Remove an installed     `ifhf remove spotify`
                          application             

  UC-03                   Search for an           `ifhf search browser`
                          application             

  UC-04                   Inspect an              `ifhf info firefox`
                          application's details   

  UC-05                   List installed          `ifhf list`
                          applications            

  UC-06                   Find installed          `ifhf list --search firefox`
                          applications            

  UC-07                   Check for updates       `ifhf updates`

  UC-08                   Update applications     `ifhf update`

  UC-09                   Refresh local metadata  `ifhf sync`

  UC-10                   Select a specific       `ifhf install firefox --remote flathub`
                          remote                  

  UC-11                   Install several         `ifhf install spotify discord firefox`
                          applications            

  UC-12                   Inspect configured      `ifhf remotes`
                          remotes                 
  -----------------------------------------------------------------------------------------

------------------------------------------------------------------------

## 3. Interface design

IFHF supports multiple command styles. These are different interfaces to
the same application logic, not separate implementations.

### 3.1 Natural command syntax

The preferred syntax for discoverability:

``` bash
ifhf install spotify
ifhf remove spotify
ifhf search browser
ifhf info firefox
ifhf list
ifhf updates
ifhf update
ifhf sync
```

### 3.2 Pacman-inspired syntax

The compact syntax for experienced users:

``` bash
ifhf -S spotify
ifhf -R spotify
ifhf -Ss browser
ifhf -Si firefox
ifhf -Q
ifhf -Qu
ifhf -Syu
```

Both syntaxes must map to the same internal command representation and
behaviour.

### 3.3 Interactive mode

Running:

``` bash
ifhf
```

may eventually open an interactive terminal interface.

Potential capabilities:

-   Search the application catalogue.
-   Inspect application details.
-   Select applications for installation or removal.
-   Review pending operations.
-   Confirm actions.

**Interactive mode is not required for the initial MVP.** It must use
the same core services as the normal CLI and must not duplicate
package-management logic.

### 3.4 Command equivalence

The following pairs must behave identically:

  Natural syntax           Compact syntax
  ------------------------ --------------------
  `ifhf install spotify`   `ifhf -S spotify`
  `ifhf remove spotify`    `ifhf -R spotify`
  `ifhf search firefox`    `ifhf -Ss firefox`
  `ifhf info firefox`      `ifhf -Si firefox`
  `ifhf list`              `ifhf -Q`
  `ifhf updates`           `ifhf -Qu`
  `ifhf update`            `ifhf -Syu`

Do not implement aliases by constructing shell command strings. Parse
both forms into a shared internal representation.

------------------------------------------------------------------------

## 4. Functional requirements

Priority labels:

-   **P0:** Required for the first usable release.
-   **P1:** Important follow-up feature.
-   **P2:** Optional enhancement.

### 4.1 Installation

**FR-001 --- Install by application name (P0)**

The user must be able to install an application using a human-readable
name.

``` bash
ifhf install spotify
```

IFHF resolves the name to an application ID and passes the selected
application to Flatpak.

**FR-002 --- Install by application ID (P0)**

Users must be able to provide an exact Flatpak application ID.

``` bash
ifhf install com.spotify.Client
```

An exact application ID should take precedence over fuzzy name matches.

**FR-003 --- Install multiple applications (P0)**

The user must be able to request multiple applications in one command.

``` bash
ifhf install spotify discord firefox
```

IFHF should resolve all requested applications before starting the
operation. It should display the planned operations and request
confirmation once.

**FR-004 --- Remote selection (P0)**

The user must be able to constrain application resolution to a specific
configured remote.

``` bash
ifhf install firefox --remote flathub
```

**FR-005 --- Installation scope (P1)**

Support user and system installation scopes.

``` bash
ifhf install firefox --user
ifhf install firefox --system
```

The default scope should be user installation unless configuration
specifies otherwise.

System installation must use Flatpak's normal permission and
authentication mechanisms.

**FR-006 --- Confirmation (P0)**

By default, IFHF must show the selected application, remote, and scope
before installation.

Support an explicit `--yes` option for non-interactive use. Never
silently select an ambiguous match.

### 4.2 Removal

**FR-007 --- Remove by name or ID (P0)**

``` bash
ifhf remove spotify
```

Resolve installed applications by exact ID or installed application
name.

**FR-008 --- Preserve application data (P0)**

Removing an application must preserve its user data by default.

**FR-009 --- Explicit data deletion (P1)**

Provide an explicit option to request deletion of application data.

``` bash
ifhf remove spotify --delete-data
```

The interface must explain the consequence before proceeding. Do not
imply that ordinary removal deletes user data.

**FR-010 --- Safe ambiguity handling (P0)**

If multiple installed applications match the supplied name, list the
candidates and require an explicit selection.

### 4.3 Search and discovery

**FR-011 --- Search applications (P0)**

``` bash
ifhf search browser
```

Search across enabled remotes by default.

**FR-012 --- Search fields (P0)**

Search should consider:

-   Application ID.
-   Application name.
-   Summary.
-   Description, where available.
-   AppStream keywords, where available.

**FR-013 --- Search result information (P0)**

Each result should display at least:

-   Application name.
-   Application ID.
-   Remote.

Include a short summary when available.

**FR-014 --- Search ranking (P0)**

Results should be ranked consistently. Exact application ID and exact
name matches must rank above partial or descriptive matches.

**FR-015 --- Empty results (P0)**

If no results are found, provide a clear message. Do not treat a failed
metadata refresh as an empty catalogue.

### 4.4 Application information

**FR-016 --- Show application details (P0)**

``` bash
ifhf info firefox
```

Display available information:

-   Name.
-   Application ID.
-   Summary and description.
-   Developer or project URL, if present.
-   Remote.
-   Branch.
-   Architecture.
-   Installed status.
-   Installation scope, if installed.

Missing metadata must be represented clearly rather than fabricated.

### 4.5 Installed applications

**FR-017 --- List installed applications (P0)**

``` bash
ifhf list
```

List installed applications and identify their installation scope and
origin where available.

**FR-018 --- Filter installed applications (P1)**

``` bash
ifhf list --search firefox
```

**FR-019 --- Distinguish applications and runtimes (P1)**

The default list should focus on user-facing applications. Provide an
option to include runtimes and other installed refs.

### 4.6 Updates

**FR-020 --- List available updates (P1)**

``` bash
ifhf updates
```

Use Flatpak's own update information. Do not invent update availability
from stale metadata.

**FR-021 --- Update applications (P1)**

``` bash
ifhf update
```

Refresh metadata when required and invoke Flatpak's update mechanism.

**FR-022 --- Update selected applications (P1)**

``` bash
ifhf update firefox
```

If supported reliably by the backend, allow updates to selected
applications.

### 4.7 Remotes

**FR-023 --- List remotes (P0)**

``` bash
ifhf remotes
```

Display configured remotes, installation scope, and enabled status where
available.

**FR-024 --- Respect user configuration (P0)**

Do not hardcode Flathub as the only source. Use the remotes configured
in the user's Flatpak installation.

### 4.8 Metadata synchronisation and cache

**FR-025 --- Refresh metadata (P1)**

``` bash
ifhf sync
```

Refresh or read AppStream metadata using supported Flatpak mechanisms.

**FR-026 --- Local cache (P1)**

IFHF may maintain a local search index to avoid repeatedly parsing
metadata.

The index must be disposable and rebuildable.

**FR-027 --- Cache invalidation (P1)**

The cache must be refreshed when relevant metadata or remote
configuration changes, or when the cache exceeds its configured age.

**FR-028 --- Offline behaviour (P1)**

Search and information commands should use available cached metadata
when offline. Clearly indicate when results may be stale.

------------------------------------------------------------------------

## 5. Package resolution and search algorithm

The resolver is a core component of IFHF and must be independently
testable.

### 5.1 Input normalisation

Before matching:

1.  Trim leading and trailing whitespace.
2.  Apply Unicode-aware case folding where practical.
3.  Normalise repeated whitespace.
4.  Preserve the original query for display.
5.  Do not modify application IDs in ways that could change their
    identity.

Punctuation handling must be conservative. Application IDs should be
compared exactly before applying name-oriented normalisation.

### 5.2 Resolution order

The resolver should consider matches in this order:

1.  Exact application ID.
2.  Exact application name.
3.  Normalised exact name.
4.  Prefix match.
5.  Token or partial-name match.
6.  Summary and keyword match.
7.  Description match.

An exact ID match must not be displaced by a fuzzy name match.

### 5.3 Ranking

A simple, deterministic scoring model is preferred over an unnecessarily
complicated search system.

An initial model could assign weights such as:

  Match type                   Example score
  -------------------------- ---------------
  Exact application ID                  1000
  Exact application name                 900
  Normalised exact name                  850
  Name prefix                            700
  All query tokens in name               600
  Partial name match                     450
  Keyword match                          300
  Summary match                          200
  Description match                      100

These are starting values, not fixed requirements. Tune them using a
representative test dataset.

Additional adjustments may be applied for:

-   Word-boundary matches.
-   Query token coverage.
-   Duplicate results from different remotes.
-   Installed status, when resolving an installed application for
    removal.

Do not automatically favour a remote simply because it is named
`flathub`.

### 5.4 Ambiguity

A result is ambiguous when multiple candidates have sufficiently similar
relevance or when an exact name maps to multiple distinct application
IDs or remotes.

The resolver must return candidates and an ambiguity state. It must not
choose arbitrarily.

Example:

``` text
$ ifhf install terminal

Multiple applications match:

1. GNOME Console
   org.gnome.Console
   Remote: flathub

2. Ptyxis
   org.gnome.Ptyxis
   Remote: flathub

Choose a result or specify its application ID.
```

### 5.5 Resolver output

The resolver should return structured data, not formatted terminal
strings.

Suggested conceptual result:

``` rust
struct ResolutionResult {
    query: String,
    candidates: Vec<ApplicationCandidate>,
    status: ResolutionStatus,
}

enum ResolutionStatus {
    Exact,
    Unique,
    Ambiguous,
    NotFound,
}
```

The exact types may change during implementation, but the separation
between resolution logic and presentation must remain.

------------------------------------------------------------------------

## 6. Metadata and local indexing

### 6.1 Metadata source

Use AppStream metadata supplied by configured Flatpak remotes and
exposed through Flatpak's supported mechanisms.

Do not build a separate online catalogue or scrape application websites.

### 6.2 Initial implementation

Start with the simplest reliable approach:

1.  Discover configured Flatpak installations and remotes.
2.  Locate or retrieve available AppStream metadata through supported
    interfaces.
3.  Parse the metadata.
4.  Convert records into an internal application model.
5.  Search the in-memory records.
6.  Add persistent indexing only if benchmarks justify it.

Do not assume Flatpak cache paths are identical across every
installation or version. Prefer supported APIs or documented locations.

### 6.3 Optional SQLite index

SQLite is local storage, not hosted infrastructure. It is optional.

Potential location:

``` text
~/.cache/ifhf/apps.db
```

Suggested schema:

``` sql
CREATE TABLE applications (
    app_id TEXT NOT NULL,
    name TEXT,
    summary TEXT,
    description TEXT,
    remote TEXT NOT NULL,
    branch TEXT,
    architecture TEXT,
    PRIMARY KEY (app_id, remote, branch, architecture)
);
```

FTS5 may be used for text search if it provides a measurable benefit.

Requirements:

-   The database is a cache, not the source of truth.
-   It can be deleted and rebuilt.
-   It must be updated transactionally.
-   A failed refresh must not corrupt the last known-good index.
-   The implementation must handle duplicate application IDs across
    remotes.
-   The index must not contain credentials or sensitive user data.

### 6.4 Cache freshness

Track when metadata was last successfully indexed and, where available,
the source metadata's modification time or version.

Do not repeatedly refresh metadata on every search.

Do not create a background daemon for cache maintenance.

------------------------------------------------------------------------

## 7. Flatpak backend

### 7.1 Backend abstraction

The CLI, resolver, and metadata components must not directly execute
Flatpak commands.

Define a backend interface that encapsulates operations such as:

-   List installations.
-   List remotes.
-   List installed applications.
-   Search or retrieve remote refs.
-   Install applications.
-   Remove applications.
-   Update applications.
-   Refresh metadata.

The initial implementation may use the Flatpak CLI.

### 7.2 Initial CLI backend

Use Rust's `std::process::Command`.

Requirements:

-   Pass arguments as separate arguments.
-   Never interpolate user input into a shell command.
-   Capture exit status and useful output.
-   Preserve relevant Flatpak error information.
-   Handle missing executables.
-   Avoid unnecessary process launches.
-   Use supported Flatpak command options.

Do not parse human-readable output when a machine-readable output mode
is available and stable.

### 7.3 Native libflatpak backend

Native libflatpak integration is a possible later improvement, not an
MVP requirement.

Before adopting it:

1.  Check the current state and maintenance of Rust bindings.
2.  Check compatibility with supported Flatpak versions.
3.  Evaluate GLib and GObject dependency requirements.
4.  Assess unsafe FFI surface area.
5.  Compare the maintenance cost against measured performance gains.

Do not introduce native bindings merely to avoid a few milliseconds of
process startup.

### 7.4 Transactions

Resolve all requested applications before starting an installation
operation.

Where the backend supports it, group multiple operations into a single
Flatpak transaction.

If a multi-application operation partially fails, report exactly which
applications succeeded and failed.

Do not claim atomicity unless the underlying backend guarantees it.

### 7.5 Scope

Support user and system installations.

Default to user scope.

Use Flatpak's normal authentication and permission mechanisms for system
operations.

------------------------------------------------------------------------

## 8. Configuration

### 8.1 Configuration location

``` text
~/.config/ifhf/config.toml
```

Use the platform's standard configuration directory resolution rather
than assuming that `$HOME` is always available.

### 8.2 Example configuration

``` toml
[general]
default_scope = "user"
default_remote = ""
confirm_install = true
confirm_remove = true
color = true

[search]
include_descriptions = true
fuzzy_matching = true

[cache]
enabled = true
max_age_hours = 24
```

### 8.3 Configuration requirements

-   Missing configuration must not prevent startup.
-   Defaults must be documented.
-   Invalid values must produce actionable errors.
-   Unknown keys should be handled predictably.
-   Configuration must never silently override explicit command-line
    options.
-   Avoid exposing dozens of options that most users will never need.

Precedence should be:

1.  Explicit command-line option.
2.  Configuration value.
3.  Built-in default.

------------------------------------------------------------------------

## 9. User experience and output

### 9.1 Design principles

-   Common commands should be easy to remember.
-   Output should be compact and readable.
-   Destructive operations should be explicit.
-   Advanced options should not clutter ordinary use.
-   Errors should explain what happened and what the user can do next.
-   Colour must not be the only way to communicate meaning.
-   Respect `NO_COLOR` and non-interactive environments.

### 9.2 Installation example

``` text
$ ifhf install spotify

Spotify
Music streaming application

Application: com.spotify.Client
Remote:      flathub
Scope:       user

Install Spotify? [Y/n]
```

### 9.3 Search example

``` text
$ ifhf search browser

Firefox
  org.mozilla.firefox
  Fast, private and open-source web browser.
  Remote: flathub

Chromium
  org.chromium.Chromium
  Open-source web browser.
  Remote: flathub
```

### 9.4 Error example

Bad:

``` text
Installation failed.
```

Good:

``` text
Could not install Firefox.

The remote 'flathub' could not be reached.

Check your network connection or inspect configured remotes:
  ifhf remotes
```

### 9.5 Non-interactive operation

When stdin is not a terminal:

-   Do not wait indefinitely for confirmation.
-   Require `--yes` where confirmation would otherwise be necessary.
-   Return a non-zero exit status if required input is missing.
-   Keep output suitable for scripts.
-   Avoid progress animations.

------------------------------------------------------------------------

## 10. Architecture

Suggested initial structure:

``` text
ifhf/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── src/
│   ├── main.rs
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── args.rs
│   │   └── commands.rs
│   ├── core/
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   ├── resolver.rs
│   │   └── service.rs
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   └── flatpak_cli.rs
│   ├── metadata/
│   │   ├── mod.rs
│   │   ├── appstream.rs
│   │   └── cache.rs
│   ├── config/
│   │   └── mod.rs
│   ├── output/
│   │   └── mod.rs
│   └── error.rs
└── tests/
    ├── cli.rs
    ├── resolver.rs
    ├── metadata.rs
    └── backend.rs
```

This is a suggested organisation, not a requirement to create empty
files. Keep the project small until its complexity justifies more
modules.

### 10.1 Layer responsibilities

**CLI layer**

-   Parse arguments.
-   Validate command shape.
-   Dispatch to application services.
-   Handle interactive confirmation.

**Core layer**

-   Application models.
-   Resolution and ranking.
-   Operation orchestration.
-   Shared application rules.

**Metadata layer**

-   AppStream discovery and parsing.
-   Local indexing.
-   Cache freshness and invalidation.

**Backend layer**

-   Flatpak operations.
-   Installation and removal.
-   Updates and remote discovery.
-   Backend-specific error conversion.

**Configuration layer**

-   Load configuration.
-   Apply defaults.
-   Validate settings.

**Output layer**

-   Format results.
-   Render progress.
-   Display actionable errors.

------------------------------------------------------------------------

## 11. Technology and dependency policy

Prefer a small, stable dependency set.

Potential dependencies:

  Crate                                          Purpose
  ---------------------------------------------- -------------------------------------
  `clap`                                         CLI parsing
  `serde`                                        Data serialisation
  `toml`                                         Configuration
  `anyhow`                                       Application-level error propagation
  `thiserror`                                    Typed library errors
  `quick-xml` or a maintained AppStream parser   XML parsing
  `rusqlite`                                     Optional local index
  `tempfile`                                     Tests

Do not add dependencies without a concrete requirement.

Avoid introducing an async runtime unless asynchronous work is genuinely
needed.

Do not implement custom HTTP downloads. Flatpak owns the network and
deployment pipeline.

Before selecting a crate, check its current maintenance status, licence,
compatibility, and API.

------------------------------------------------------------------------

## 12. Performance goals and measurement

Performance is a design objective, but claims must be based on
measurements.

### 12.1 Initial targets

These are aspirational targets for a warm local cache, not guaranteed
results.

  Operation                         Initial target
  ------------------------------- ----------------
  Warm CLI startup                     Under 20 ms
  Cached search                        Under 20 ms
  Cached exact-name resolution         Under 20 ms
  Installed application listing        Under 30 ms

The target machine, Rust build profile, cache state, and measurement
method must be documented.

### 12.2 Benchmarks

Use `hyperfine` or an equivalent repeatable benchmarking tool.

Examples:

``` bash
hyperfine 'flatpak list' 'ifhf list'
```

``` bash
hyperfine 'flatpak search firefox' 'ifhf search firefox'
```

Measure:

-   Cold startup.
-   Warm startup.
-   First metadata load.
-   Cached search.
-   Exact ID resolution.
-   Name resolution.
-   Installed application listing.

Do not compare commands that perform different amounts of work.

### 12.3 Realistic performance claims

IFHF may reduce:

-   CLI startup overhead.
-   Repeated metadata parsing.
-   Search and resolution latency.
-   Unnecessary subprocess launches.
-   Repeated work during multi-package operations.

IFHF cannot independently make Flatpak downloads or OSTree deployments
substantially faster.

Application launch performance is outside IFHF's scope.

------------------------------------------------------------------------

## 13. Security and reliability

### 13.1 Security requirements

-   Preserve Flatpak signature verification.
-   Preserve Flatpak's permission and authentication model.
-   Never execute user input through a shell.
-   Never bypass system permissions.
-   Never manipulate OSTree repository internals.
-   Never implement an alternative download or deployment path.
-   Do not store credentials.
-   Validate remote names and application identifiers where appropriate.
-   Do not request elevated privileges unless the requested operation
    requires them.

### 13.2 Reliability requirements

-   Handle missing Flatpak installations.
-   Handle missing or disabled remotes.
-   Handle network failures.
-   Handle malformed metadata.
-   Handle stale caches.
-   Handle duplicate application IDs across remotes.
-   Handle partial failures in multi-application operations.
-   Preserve the last known-good metadata index if a refresh fails.
-   Return meaningful exit codes.

------------------------------------------------------------------------

## 14. Testing strategy

Tests must not modify the developer's real Flatpak installation.

Use mock backend implementations and temporary metadata fixtures for
routine tests.

### 14.1 Unit tests

Test:

1.  Query normalisation.
2.  Exact application ID matching.
3.  Exact name matching.
4.  Partial matching.
5.  Ranking order.
6.  Ambiguous matches.
7.  Duplicate application IDs across remotes.
8.  Missing metadata fields.
9.  Invalid application records.
10. Cache freshness decisions.
11. Configuration defaults.
12. Invalid configuration.

### 14.2 Integration tests

Test:

1.  CLI argument parsing.
2.  Natural and compact syntax equivalence.
3.  Backend argument construction.
4.  Missing Flatpak executable.
5.  Failed installation.
6.  Failed removal.
7.  Multi-application operations.
8.  Non-interactive confirmation behaviour.
9.  User and system scope selection.
10. Partial operation failures.

### 14.3 Acceptance tests

The first usable release must demonstrate that:

-   A user can search for an application by ordinary name.
-   A user can install an application without knowing its ID.
-   Ambiguous results are never silently selected.
-   A user can remove an application without deleting its data by
    default.
-   Installed applications can be listed.
-   Multiple applications can be requested in one command.
-   Flatpak remains responsible for actual deployment.
-   The application works without any IFHF-operated service.

------------------------------------------------------------------------

## 15. Development roadmap

Development should be incremental. Each phase must leave the project in
a working state.

### Phase 0 --- Project validation

-   Inspect the installed Flatpak version and environment.
-   Check current Rust crate APIs and compatibility.
-   Confirm how configured installations and remotes will be discovered.
-   Create a minimal Cargo project.
-   Record initial design decisions.

**Deliverable:** A compiling Rust project and a validated technical
approach.

### Phase 1 --- Core CLI

Implement:

-   Natural command syntax.
-   Compact command syntax.
-   Shared internal command representation.
-   Install.
-   Remove.
-   List installed applications.
-   Flatpak CLI backend.
-   Basic error handling.
-   Mock backend tests.

**Deliverable:** A usable CLI for basic package management.

### Phase 2 --- Search and resolution

Implement:

-   AppStream discovery.
-   Metadata parsing.
-   Application model.
-   Search.
-   Deterministic ranking.
-   Ambiguity handling.
-   Application information.
-   Multi-remote support.

**Deliverable:** Applications can be discovered and resolved by name.

### Phase 3 --- Reliability and configuration

Implement:

-   User/system scope.
-   Confirmation rules.
-   Non-interactive behaviour.
-   Configuration file.
-   Better error reporting.
-   Multi-package failure reporting.

**Deliverable:** A reliable daily-use CLI.

### Phase 4 --- Updates and caching

Implement:

-   Update listing.
-   Update operations.
-   Metadata synchronisation.
-   Cache freshness.
-   Optional local SQLite index, only if justified.

**Deliverable:** A complete everyday package-management workflow.

### Phase 5 --- Performance evaluation

-   Benchmark against equivalent Flatpak commands.
-   Identify actual bottlenecks.
-   Optimise measured hot paths.
-   Evaluate whether native libflatpak integration is worthwhile.

**Deliverable:** Repeatable benchmark results and justified optimisation
decisions.

### Phase 6 --- Release preparation

-   Documentation.
-   Installation instructions.
-   Packaging.
-   Automated tests.
-   Versioning.
-   Changelog.
-   Release builds.

**Deliverable:** A maintainable public release.

------------------------------------------------------------------------

## 16. Explicit scope limits

The following should not be implemented unless a later, separately
justified project phase requires them:

-   Native libflatpak bindings in the first working version.
-   A custom HTTP client.
-   A custom download manager.
-   A hosted database.
-   A hosted API.
-   A custom application repository.
-   A background daemon.
-   Automatic permission changes.
-   A graphical application store.
-   A custom package dependency resolver.
-   Direct OSTree manipulation.
-   A plugin system.
-   A general-purpose package manager for non-Flatpak formats.

The purpose is to make Flatpak easier to manage, not to recreate the
entire Linux software distribution ecosystem.

------------------------------------------------------------------------

## 17. Definition of done: IFHF 1.0

IFHF 1.0 is ready when it can:

-   Install applications by name or ID.
-   Remove applications safely.
-   Search configured remotes.
-   Resolve names deterministically.
-   Detect ambiguous matches.
-   Show application information.
-   List installed applications.
-   List and apply updates.
-   Handle multiple applications in one operation.
-   Support user and system installations.
-   Reuse existing Flatpak metadata.
-   Work without IFHF-operated infrastructure.
-   Preserve Flatpak's security guarantees.
-   Provide useful errors and reliable exit codes.
-   Pass its automated test suite.
-   Include clear documentation.

Performance claims must be supported by reproducible benchmarks.

------------------------------------------------------------------------

## 18. Instructions for a coding agent

Implement this specification incrementally.

### Before implementation

1.  Inspect the repository and development environment.
2.  Check the installed Flatpak version and available commands.
3.  Verify current crate APIs and compatibility.
4.  Identify the simplest supported way to access AppStream metadata.
5.  Propose the initial module structure.
6.  Record any deviations from this specification before implementing
    them.

### Implementation rules

-   Start with Phase 0 and Phase 1 only.
-   Do not implement the entire roadmap in one pass.
-   Keep the first version small and functional.
-   Use a backend abstraction so Flatpak integration can change later.
-   Use mocks for tests; do not modify the host's real Flatpak
    installation during routine testing.
-   Do not introduce SQLite before measuring the simpler metadata
    approach.
-   Do not introduce native libflatpak bindings before measuring
    subprocess overhead.
-   Do not add custom network or download logic.
-   Do not silently alter command semantics.
-   Do not add features merely because they are technically interesting.
-   Keep code readable and document non-obvious decisions.
-   Run formatting, linting, and tests after each meaningful
    implementation step.

### Required first milestone

The first milestone is complete when:

``` bash
ifhf install spotify
ifhf -S spotify

ifhf remove spotify
ifhf -R spotify

ifhf list
ifhf -Q
```

all reach the same underlying application logic for their respective
operations, with safe argument handling, useful errors, and tests.

**Do not proceed to metadata indexing or native bindings until this
milestone is complete and reviewed.**
