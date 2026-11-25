# IFHF vs Flatpak benchmarks

## Environment

- Date: 2026-11-25
- CPU: Intel(R) Core(TM) i5-10310U CPU @ 1.70GHz (8 threads)
- Kernel: 7.2.11-300.fc45.x86_64
- Flatpak: Flatpak 1.18.4
- ifhf: ifhf 0.1.0 (release build: opt-level 3, LTO, panic=abort, stripped)
- hyperfine: hyperfine 1.20.0
- Installed refs: 26; flathub catalogue entries indexed: 3978

Method: `hyperfine`, mean ± σ over the stated runs; `-N` (no shell) for fast commands. Both tools write to a pipe that hyperfine drains (`--output pipe`), so neither gets a free `/dev/null`. Each pair performs the same work (e.g. `ifhf search --limit 0` prints every match like `flatpak search` does). Warm = OS page cache warm and ifhf index present. ifhf's index lives in an isolated cache directory so these runs do not disturb your real one.

## Startup (`--version`)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak --version` | 25.9 ms ± 1.6 ms | 25.1 ms … 37.4 ms | 100 |
| `ifhf --version` | 1.5 ms ± 0.0 ms | 1.4 ms … 1.5 ms | 100 |

**ifhf is 17.5× faster**

## List installed applications

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak list --app --columns=name,application,branch,installation,origin` | 31.5 ms ± 4.9 ms | 18.5 ms … 35.9 ms | 50 |
| `ifhf list` | 31.4 ms ± 7.1 ms | 20.0 ms … 37.9 ms | 50 |

**equivalent** (within 5%)

## List configured remotes

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remotes --show-disabled --columns=name,options,url` | 25.0 ms ± 5.8 ms | 16.5 ms … 33.4 ms | 50 |
| `ifhf remotes` | 32.2 ms ± 1.1 ms | 29.0 ms … 34.7 ms | 50 |

**flatpak is 1.3× faster** (ifhf overhead ≈ 7.2 ms)

## Search `firefox` (ifhf: warm index; flatpak has no persistent index)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak search firefox` | 2.35 s ± 38.8 ms | 2.30 s … 2.40 s | 5 |
| `ifhf search firefox --limit 0` | 11.1 ms ± 0.7 ms | 10.1 ms … 11.7 ms | 5 |

**ifhf is 211.6× faster**

## Search `browser` (many matches)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak search browser` | 2.32 s ± 28.3 ms | 2.28 s … 2.35 s | 5 |
| `ifhf search browser --limit 0` | 10.8 ms ± 0.5 ms | 10.4 ms … 11.6 ms | 5 |

**ifhf is 214.4× faster**

## First metadata load (ifhf: index deleted before every run)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search firefox --limit 0` | 532.1 ms ± 25.2 ms | 503.5 ms … 572.9 ms | 10 |

## Show application details by exact ID (remote metadata)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remote-info flathub com.spotify.Client` | 1.48 s ± 126.4 ms | 1.38 s … 1.75 s | 10 |
| `ifhf info com.spotify.Client` | 49.8 ms ± 2.4 ms | 46.2 ms … 53.9 ms | 10 |

**ifhf is 29.7× faster**

## Show details of an installed application

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak info com.spotify.Client` | 30.8 ms ± 4.3 ms | 20.9 ms … 37.5 ms | 30 |
| `ifhf info com.spotify.Client` | 54.8 ms ± 2.2 ms | 52.7 ms … 61.5 ms | 30 |

**flatpak is 1.8× faster** (ifhf overhead ≈ 24.1 ms)

## ifhf only: exact-ID resolution (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search org.mozilla.firefox --limit 0` | 22.4 ms ± 5.2 ms | 10.9 ms … 36.6 ms | 100 |

## ifhf only: name resolution (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search spotify --limit 0` | 24.3 ms ± 1.4 ms | 22.4 ms … 30.0 ms | 100 |

## ifhf only: multi-word keyword search (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search web browser --limit 0` | 26.3 ms ± 3.0 ms | 23.6 ms … 38.3 ms | 100 |

## Check for updates (network)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remote-ls --updates` | 1.97 s ± 40.8 ms | 1.92 s … 2.03 s | 5 |
| `ifhf updates` | 2.04 s ± 153.5 ms | 1.94 s … 2.31 s | 5 |

**equivalent** (within 5%)

## Install one app (network, isolated user installation)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak install --user -y flathub io.github.hrkfdn.ncspot` | 1.34 s ± 287.2 ms | 1.04 s … 1.73 s | 5 |
| `ifhf install ncspot --yes` | 1.17 s ± 216.7 ms | 1.01 s … 1.52 s | 5 |

**ifhf is 1.1× faster**

## Remove one app (isolated user installation)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak uninstall --user -y io.github.hrkfdn.ncspot` | 192.8 ms ± 47.0 ms | 109.4 ms … 219.5 ms | 5 |
| `ifhf remove ncspot --yes` | 207.7 ms ± 42.3 ms | 147.0 ms … 249.1 ms | 5 |

**flatpak is 1.1× faster** (ifhf overhead ≈ 14.8 ms)

## Peak memory

| Command | Peak RSS (KiB) |
|:---|---:|
| `flatpak search firefox` | 449836 |
| `ifhf search firefox --limit 0` (warm index) | 19844 |
| `flatpak list --app` | 19700 |
| `ifhf list` | 19336 |

## Binary

`ifhf` release binary: 969 KiB.

