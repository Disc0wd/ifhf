#!/usr/bin/env python3
"""Turn hyperfine JSON into bench/RESULTS.md: environment header + one table per benchmark."""
import json, os, platform, subprocess, sys, glob, datetime

out, ifhf = sys.argv[1], sys.argv[2]

def sh(cmd):
    try:
        return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip()
    except Exception:
        return "?"

def fmt(sec):
    ms = sec * 1000
    return f"{ms/1000:.2f} s" if ms >= 1000 else f"{ms:.1f} ms"

TITLES = {
    "01_startup": "Startup (`--version`)",
    "02_list_installed": "List installed applications",
    "03_remotes": "List configured remotes",
    "04_search_warm_index": "Search `firefox` (ifhf: warm index; flatpak has no persistent index)",
    "05_search_broad_term_warm_index": "Search `browser` (many matches)",
    "06_first_metadata_load_cold_index": "First metadata load (ifhf: index deleted before every run)",
    "07_info_exact_id": "Show application details by exact ID (remote metadata)",
    "08_installed_info": "Show details of an installed application",
    "09_ifhf_exact_id_resolution": "ifhf only: exact-ID resolution (warm)",
    "10_ifhf_name_resolution": "ifhf only: name resolution (warm)",
    "11_ifhf_keyword_resolution": "ifhf only: multi-word keyword search (warm)",
    "12_updates_check": "Check for updates (network)",
    "13_install_one_app": "Install one app (network, isolated user installation)",
    "14_remove_one_app": "Remove one app (isolated user installation)",
}

lines = ["# IFHF vs Flatpak benchmarks", ""]
cpu = sh("grep -m1 'model name' /proc/cpuinfo | cut -d: -f2").strip()
lines += [
    "## Environment", "",
    f"- Date: {datetime.date.today()}",
    f"- CPU: {cpu or platform.processor()} ({os.cpu_count()} threads)",
    f"- Kernel: {platform.release()}",
    f"- Flatpak: {sh('flatpak --version')}",
    f"- ifhf: {sh(ifhf + ' --version')} (release build: opt-level 3, LTO, panic=abort, stripped)",
    f"- hyperfine: {sh('hyperfine --version')}",
    f"- Installed refs: {sh('flatpak list | wc -l')}; flathub catalogue entries indexed: "
    f"{sh('cd /tmp && XDG_CACHE_HOME=$(mktemp -d) ' + ifhf + ' search . --limit 0 2>/dev/null | grep -c Remote:')}",
    "",
    "Method: `hyperfine`, mean ± σ over the stated runs; `-N` (no shell) for fast commands. "
    "Output is discarded by hyperfine for both tools. Each pair performs the same work "
    "(e.g. `ifhf search --limit 0` prints every match like `flatpak search` does). "
    "Warm = OS page cache warm and ifhf index present. ifhf's index lives in an isolated cache "
    "directory so these runs do not disturb your real one.", "",
]

for jf in sorted(glob.glob(os.path.join(out, "*.json"))):
    key = os.path.basename(jf)[:-5]
    data = json.load(open(jf))["results"]
    lines += [f"## {TITLES.get(key, key)}", ""]
    lines += ["| Command | Mean ± σ | Min … Max | Runs |", "|:---|---:|---:|---:|"]
    for r in data:
        cmd = r["command"].replace(os.path.dirname(os.path.dirname(os.path.abspath(out))) + "/target/release/", "")
        lines.append(f"| `{cmd}` | {fmt(r['mean'])} ± {fmt(r['stddev'])} | {fmt(r['min'])} … {fmt(r['max'])} | {len(r['times'])} |")
    if len(data) == 2:
        a, b = data
        fl, ifh = (a, b) if "flatpak" in a["command"].split()[0] else (b, a)
        ratio = fl["mean"] / ifh["mean"]
        if ratio >= 1.05:
            verdict = f"**ifhf is {ratio:.1f}× faster**"
        elif ratio <= 1 / 1.05:
            verdict = f"**flatpak is {1/ratio:.1f}× faster** (ifhf overhead ≈ {fmt(ifh['mean'] - fl['mean'])})"
        else:
            verdict = "**equivalent** (within 5%)"
        lines += ["", verdict]
    lines.append("")

mem = os.path.join(out, "15_memory.md")
if os.path.exists(mem):
    lines += ["## Peak memory", "", open(mem).read().strip(), ""]
size = os.path.getsize(ifhf) // 1024
lines += ["## Binary", "", f"`ifhf` release binary: {size} KiB.", ""]
print("\n".join(lines))
