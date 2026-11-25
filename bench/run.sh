#!/usr/bin/env bash
# Benchmark ifhf against Flatpak's own CLI. Needs: hyperfine, flatpak, a release build of ifhf.
# Read-only against your real installations, except the install/remove cycle, which runs in a
# throwaway FLATPAK_USER_DIR (set IFHF_BENCH_NETWORK=0 to skip all network-bound benchmarks).
#
#   cargo build --release && bench/run.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IFHF="$ROOT/target/release/ifhf"
OUT="$ROOT/bench/results"
WORK="$(mktemp -d)"
NET="${IFHF_BENCH_NETWORK:-1}"
trap 'rm -rf "$WORK"' EXIT
[ -x "$IFHF" ] || { echo "build first: cargo build --release" >&2; exit 1; }
command -v hyperfine >/dev/null || { echo "hyperfine not found" >&2; exit 1; }
mkdir -p "$OUT"; rm -f "$OUT"/*.md "$OUT"/*.json

export XDG_CACHE_HOME="$WORK/cache"      # isolate ifhf's index so cold/warm is under our control
APP_ID=io.github.hrkfdn.ncspot            # small app that runs on an already-installed runtime

bench() { # name, runs, [hyperfine args...] -- commands...
  local name="$1" runs="$2"; shift 2
  echo "=== $name"
  hyperfine --runs "$runs" --style basic --output pipe --export-json "$OUT/$name.json" \
            --export-markdown "$OUT/$name.md" "$@" >/dev/null
  cat "$OUT/$name.md"
}

# Prime the index so "warm" really is warm.
"$IFHF" search firefox >/dev/null 2>&1 || true

# ---- local, read-only -----------------------------------------------------------------
bench 01_startup 100 --warmup 10 -N "flatpak --version" "$IFHF --version"
bench 02_list_installed 50 --warmup 5 -N \
  "flatpak list --app --columns=name,application,branch,installation,origin" "$IFHF list"
bench 03_remotes 50 --warmup 5 -N \
  "flatpak remotes --show-disabled --columns=name,options,url" "$IFHF remotes"
bench 04_search_warm_index 5 --warmup 1 -N \
  "flatpak search firefox" "$IFHF search firefox --limit 0"
bench 05_search_broad_term_warm_index 5 --warmup 1 -N \
  "flatpak search browser" "$IFHF search browser --limit 0"
bench 06_first_metadata_load_cold_index 10 --warmup 0 \
  --prepare "rm -rf $XDG_CACHE_HOME/ifhf" "$IFHF search firefox --limit 0"
bench 07_info_exact_id 10 --warmup 1 -N \
  "flatpak remote-info flathub com.spotify.Client" "$IFHF info com.spotify.Client"
bench 08_installed_info 30 --warmup 3 -N \
  "flatpak info com.spotify.Client" "$IFHF info com.spotify.Client"
# ifhf-only: flatpak cannot resolve a name or a keyword, so there is nothing equivalent to compare.
bench 09_ifhf_exact_id_resolution 100 --warmup 5 -N "$IFHF search org.mozilla.firefox --limit 0"
bench 10_ifhf_name_resolution 100 --warmup 5 -N "$IFHF search spotify --limit 0"
bench 11_ifhf_keyword_resolution 100 --warmup 5 -N "$IFHF search web browser --limit 0"

# ---- network-bound --------------------------------------------------------------------
if [ "$NET" = "1" ]; then
  bench 12_updates_check 5 --warmup 1 -N "flatpak remote-ls --updates" "$IFHF updates"

  # Install/remove in an isolated user installation. System runtimes are reused.
  export FLATPAK_USER_DIR="$WORK/user"
  flatpak remote-add --user flathub https://dl.flathub.org/repo/flathub.flatpakrepo >/dev/null
  flatpak update --appstream --user >/dev/null 2>&1
  "$IFHF" search ncspot >/dev/null
  FP_RM="flatpak uninstall --user -y $APP_ID"
  FP_IN="flatpak install --user -y flathub $APP_ID"
  bench 13_install_one_app 5 --warmup 0 --prepare "$FP_RM >/dev/null 2>&1 || true" \
    "$FP_IN" "$IFHF install ncspot --yes"
  bench 14_remove_one_app 5 --warmup 0 --prepare "$FP_IN >/dev/null 2>&1 || true" \
    "$FP_RM" "$IFHF remove ncspot --yes"
  $FP_RM >/dev/null 2>&1 || true
  unset FLATPAK_USER_DIR
fi

# ---- memory and size ------------------------------------------------------------------
rss() { /usr/bin/time -f '%M' "$@" 2>&1 >/dev/null | tail -1; }
{
  echo "| Command | Peak RSS (KiB) |"
  echo "|:---|---:|"
  echo "| \`flatpak search firefox\` | $(rss flatpak search firefox) |"
  echo "| \`ifhf search firefox --limit 0\` (warm index) | $(rss "$IFHF" search firefox --limit 0) |"
  echo "| \`flatpak list --app\` | $(rss flatpak list --app) |"
  echo "| \`ifhf list\` | $(rss "$IFHF" list) |"
} > "$OUT/15_memory.md" 2>&1 || true
cat "$OUT/15_memory.md"

# ---- summary --------------------------------------------------------------------------
python3 "$ROOT/bench/summarize.py" "$OUT" "$IFHF" > "$ROOT/bench/RESULTS.md"
echo "wrote bench/RESULTS.md"
