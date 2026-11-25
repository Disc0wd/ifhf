# IFHF vs Flatpak benchmarks

## Environment

- Date: 2026-11-25
- CPU: Intel(R) Core(TM) i5-10310U CPU @ 1.70GHz (8 threads)
- Kernel: 7.2.11-300.fc45.x86_64
- Flatpak: Flatpak 1.18.4
- ifhf: ifhf 0.1.0 (release build: opt-level 3, LTO, panic=abort, stripped)
- hyperfine: hyperfine 1.20.0
- Installed refs: 25; flathub catalogue entries indexed: 3978

Method: `hyperfine`, mean ± σ over the stated runs; `-N` (no shell) for fast commands. Output is discarded by hyperfine for both tools. Each pair performs the same work (e.g. `ifhf search --limit 0` prints every match like `flatpak search` does). Warm = OS page cache warm and ifhf index present. ifhf's index lives in an isolated cache directory so these runs do not disturb your real one.

## Startup (`--version`)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak --version` | 32.5 ms ± 4.9 ms | 18.1 ms … 43.7 ms | 100 |
| `ifhf --version` | 2.0 ms ± 0.2 ms | 1.7 ms … 3.2 ms | 100 |

**ifhf is 16.6× faster**

## List installed applications

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak list --app --columns=name,application,branch,installation,origin` | 33.4 ms ± 9.5 ms | 19.7 ms … 45.4 ms | 50 |
| `ifhf list` | 48.7 ms ± 9.6 ms | 24.8 ms … 67.9 ms | 50 |

**flatpak is 1.5× faster** (ifhf overhead ≈ 15.3 ms)

## List configured remotes

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remotes --show-disabled --columns=name,options,url` | 29.1 ms ± 8.2 ms | 17.7 ms … 39.4 ms | 50 |
| `ifhf remotes` | 36.5 ms ± 7.5 ms | 18.9 ms … 50.6 ms | 50 |

**flatpak is 1.3× faster** (ifhf overhead ≈ 7.4 ms)

## Search `firefox` (ifhf: warm index; flatpak has no persistent index)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak search firefox` | 2.69 s ± 91.9 ms | 2.59 s … 2.81 s | 5 |
| `ifhf search firefox --limit 0` | 13.3 ms ± 1.6 ms | 11.9 ms … 15.6 ms | 5 |

**ifhf is 201.9× faster**

## Search `browser` (many matches)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak search browser` | 2.63 s ± 53.4 ms | 2.56 s … 2.69 s | 5 |
| `ifhf search browser --limit 0` | 12.5 ms ± 0.6 ms | 11.9 ms … 13.1 ms | 5 |

**ifhf is 210.9× faster**

## First metadata load (ifhf: index deleted before every run)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search firefox --limit 0` | 577.3 ms ± 33.7 ms | 536.5 ms … 650.2 ms | 10 |

## Show application details by exact ID (remote metadata)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remote-info flathub com.spotify.Client` | 1.56 s ± 49.9 ms | 1.49 s … 1.64 s | 10 |
| `ifhf info com.spotify.Client` | 58.2 ms ± 23.0 ms | 36.9 ms … 91.0 ms | 10 |

**ifhf is 26.8× faster**

## Show details of an installed application

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak info com.spotify.Client` | 23.7 ms ± 5.8 ms | 19.1 ms … 37.3 ms | 30 |
| `ifhf info com.spotify.Client` | 67.1 ms ± 15.5 ms | 38.2 ms … 88.0 ms | 30 |

**flatpak is 2.8× faster** (ifhf overhead ≈ 43.4 ms)

## ifhf only: exact-ID resolution (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search org.mozilla.firefox --limit 0` | 27.0 ms ± 8.7 ms | 10.6 ms … 67.4 ms | 100 |

## ifhf only: name resolution (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search spotify --limit 0` | 28.6 ms ± 1.5 ms | 26.5 ms … 34.2 ms | 100 |

## ifhf only: multi-word keyword search (warm)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `ifhf search web browser --limit 0` | 33.4 ms ± 3.1 ms | 27.6 ms … 45.7 ms | 100 |

## Check for updates (network)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak remote-ls --updates` | 2.24 s ± 49.2 ms | 2.17 s … 2.30 s | 5 |
| `ifhf updates` | 2.36 s ± 176.0 ms | 2.16 s … 2.58 s | 5 |

**flatpak is 1.1× faster** (ifhf overhead ≈ 121.5 ms)

## Install one app (network, isolated user installation)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak install --user -y flathub io.github.hrkfdn.ncspot` | 1.72 s ± 777.0 ms | 1.27 s … 3.09 s | 5 |
| `ifhf install ncspot --yes` | 1.30 s ± 139.0 ms | 1.12 s … 1.47 s | 5 |

**ifhf is 1.3× faster**

## Remove one app (isolated user installation)

| Command | Mean ± σ | Min … Max | Runs |
|:---|---:|---:|---:|
| `flatpak uninstall --user -y io.github.hrkfdn.ncspot` | 224.7 ms ± 51.7 ms | 152.4 ms … 275.3 ms | 5 |
| `ifhf remove ncspot --yes` | 309.1 ms ± 33.3 ms | 264.5 ms … 352.1 ms | 5 |

**flatpak is 1.4× faster** (ifhf overhead ≈ 84.4 ms)

## Peak memory

| Command | Peak RSS (KiB) |
|:---|---:|
| `flatpak search firefox` | 449444 |
| `ifhf search firefox --limit 0` (warm index) | 19744 |
| `flatpak list --app` | 19016 |
| `ifhf list` | 19380 |

## Binary

`ifhf` release binary: 957 KiB.

