#!/usr/bin/env python3
"""Generate docs/index.html (GitHub Pages) from bench/results/*.json.

Usage: make_site.py [repo-url]
Every number on the page comes from the benchmark JSON; nothing is typed in by hand.
"""
import html
import json
import math
import os
import sys

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
repo = sys.argv[1] if len(sys.argv) > 1 else "#"
res = os.path.join(root, "bench", "results")


def load(key):
    return json.load(open(os.path.join(res, key + ".json")))["results"]


def pair(key):
    d = load(key)
    fl = next(r for r in d if r["command"].startswith("flatpak"))
    ih = next(r for r in d if "ifhf" in r["command"].split()[0])
    return fl["mean"], ih["mean"]


def fmt(sec):
    ms = sec * 1000
    if ms >= 1000:
        return f"{sec:.2f} s"
    return f"{ms:.0f} ms" if ms >= 10 else f"{ms:.1f} ms"


def log_pct(sec):
    """Position on a log axis from 1 ms (0%) to 10 s (100%)."""
    ms = max(sec * 1000, 1.0)
    return min(math.log10(ms) / 4 * 100, 100)


ROWS = [
    ("04_search_warm_index", "Search “firefox”"),
    ("05_search_broad_term_warm_index", "Search “browser”"),
    ("07_info_exact_id", "Details of an app, by ID"),
    ("01_startup", "Start the program"),
    ("12_updates_check", "Check for updates"),
    ("13_install_one_app", "Install one app"),
    ("02_list_installed", "List installed apps"),
    ("03_remotes", "List remotes"),
    ("14_remove_one_app", "Remove one app"),
    ("08_installed_info", "Info on an installed app"),
]

rows = []
for key, title in ROWS:
    fl, ih = pair(key)
    ratio = fl / ih
    if ratio >= 1.25:
        verdict = f"{ratio:.0f}× faster" if ratio >= 10 else f"{ratio:.1f}× faster"
        cls = "win"
    elif ratio <= 0.8:
        verdict, cls = f"{1 / ratio:.1f}× slower", "lose"
    else:
        verdict, cls = "about equal", "even"
    label = f"{title}: flatpak {fmt(fl)}, ifhf {fmt(ih)}, ifhf is {verdict}"
    rows.append(
        f"""<li class="row {cls}">
<div class="task">{html.escape(title)}</div>
<div class="track" role="img" aria-label="{html.escape(label)}"><div class="inner">
<div class="line"><i class="bar fl" style="width:{log_pct(fl):.1f}%"></i><span class="val" style="left:{log_pct(fl):.1f}%">{fmt(fl)}</span></div>
<div class="line"><i class="bar ih" style="width:{log_pct(ih):.1f}%"></i><span class="val" style="left:{log_pct(ih):.1f}%">{fmt(ih)}</span></div>
</div></div>
<div class="verdict">{verdict}</div>
</li>"""
    )

fl_search, ih_search = pair("04_search_warm_index")
cold = load("06_first_metadata_load_cold_index")[0]["mean"]
mem = {}
for line in open(os.path.join(res, "15_memory.md")):
    if "flatpak search firefox" in line:
        mem["fl"] = int(line.split("|")[2])
    if "ifhf search firefox" in line:
        mem["ih"] = int(line.split("|")[2])

TEMPLATE = open(os.path.join(root, "bench", "site_template.html")).read()
page = (
    TEMPLATE.replace("@@REPO@@", repo)
    .replace("@@ROWS@@", "\n".join(rows))
    .replace("@@FL_SEC@@", f"{fl_search:.2f}")
    .replace("@@FL_MS@@", f"{fl_search * 1000:.0f}")
    .replace("@@IH_LABEL@@", fmt(ih_search))
    .replace("@@IH_MS@@", f"{ih_search * 1000:.1f}")
    .replace("@@FL_LABEL@@", fmt(fl_search))
    .replace("@@COLD@@", fmt(cold))
    .replace("@@MEM_FL@@", f"{mem['fl'] / 1024:.0f} MiB")
    .replace("@@MEM_IH@@", f"{mem['ih'] / 1024:.0f} MiB")
    .replace("@@SPEEDUP@@", f"{fl_search / ih_search:.0f}")
)
assert "@@" not in page, "unreplaced placeholder"
os.makedirs(os.path.join(root, "docs"), exist_ok=True)
open(os.path.join(root, "docs", "index.html"), "w").write(page)
print("wrote docs/index.html")
