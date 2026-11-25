#!/usr/bin/env python3
"""Generate docs/index.html (GitHub Pages) from bench/results/*.json. Usage: make_site.py [repo-url]"""
import html, json, os, sys

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
repo = sys.argv[1] if len(sys.argv) > 1 else "#"
res = os.path.join(root, "bench", "results")

def load(key):
    return json.load(open(os.path.join(res, key + ".json")))["results"]

def fmt(sec):
    ms = sec * 1000
    return f"{sec:.2f} s" if ms >= 1000 else (f"{ms:.0f} ms" if ms >= 10 else f"{ms:.1f} ms")

ROWS = [
    ("04_search_warm_index", "Search “firefox”"),
    ("05_search_broad_term_warm_index", "Search “browser”"),
    ("07_info_exact_id", "Details by exact ID"),
    ("01_startup", "Startup"),
    ("02_list_installed", "List installed apps"),
    ("03_remotes", "List remotes"),
    ("12_updates_check", "Check for updates"),
    ("13_install_one_app", "Install one app"),
    ("14_remove_one_app", "Remove one app"),
    ("08_installed_info", "Info on an installed app"),
]

rows = []
for key, title in ROWS:
    d = load(key)
    fl = next(r for r in d if r["command"].startswith("flatpak"))
    ih = next(r for r in d if "ifhf" in r["command"].split()[0])
    ratio = fl["mean"] / ih["mean"]
    if ratio >= 1.25:
        verdict, cls = f"{ratio:.0f}× faster" if ratio >= 10 else f"{ratio:.1f}× faster", "win"
    elif ratio <= 0.8:
        verdict, cls = f"{1/ratio:.1f}× slower", "lose"
    else:
        verdict, cls = "≈ equal", "even"
    top = max(fl["mean"], ih["mean"])
    rows.append(f"""<tr class="{cls}"><th scope="row">{html.escape(title)}</th>
<td><span class="bar fl" style="width:{max(fl['mean']/top*100, 1.5):.1f}%"></span><span class="v">{fmt(fl['mean'])}</span></td>
<td><span class="bar ih" style="width:{max(ih['mean']/top*100, 1.5):.1f}%"></span><span class="v">{fmt(ih['mean'])}</span></td>
<td class="verdict">{verdict}</td></tr>""")

cold = load("06_first_metadata_load_cold_index")[0]["mean"]

page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>IFHF — a friendly command line for Flatpak</title>
<meta name="description" content="Install, remove and search Flatpak apps by ordinary name. Searches in milliseconds, not seconds.">
<style>
:root{{--bg:#fbfaf8;--fg:#1d1d1f;--mute:#6b6b70;--line:#e4e1db;--card:#fff;--acc:#1f6f5c;--fl:#b9b4aa;--ih:#1f6f5c;--win:#1f6f5c;--lose:#b3412c;--code:#f1eee8}}
@media(prefers-color-scheme:dark){{:root{{--bg:#151617;--fg:#ecebe8;--mute:#9a9a9f;--line:#2b2d2f;--card:#1b1d1e;--acc:#5fcfb2;--fl:#55524c;--ih:#5fcfb2;--win:#5fcfb2;--lose:#ef8a73;--code:#202223}}}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--bg);color:var(--fg);font:16px/1.6 system-ui,-apple-system,"Segoe UI",sans-serif}}
main{{max-width:60rem;margin:0 auto;padding:3rem 1.25rem 5rem}}
h1{{font-size:clamp(2.4rem,7vw,4rem);margin:.2em 0 .1em;letter-spacing:-.03em;line-height:1}}
h2{{margin:3.2rem 0 .8rem;font-size:1.5rem;letter-spacing:-.01em}}
.lede{{font-size:1.25rem;color:var(--mute);max-width:40rem;margin:.4rem 0 1.4rem}}
a{{color:var(--acc)}}
.cta a{{display:inline-block;margin:0 .6rem .6rem 0;padding:.55rem 1rem;border:1px solid var(--line);border-radius:.5rem;text-decoration:none;font-weight:600}}
.cta a.p{{background:var(--acc);color:var(--bg);border-color:var(--acc)}}
pre,code{{font:.92em ui-monospace,SFMono-Regular,Menlo,monospace}}
pre{{background:var(--code);border:1px solid var(--line);border-radius:.6rem;padding:1rem 1.1rem;overflow:auto;margin:1rem 0}}
code{{background:var(--code);padding:.1em .35em;border-radius:.3em}} pre code{{background:none;padding:0}}
table{{width:100%;border-collapse:collapse;background:var(--card);border:1px solid var(--line);border-radius:.6rem;overflow:hidden}}
th,td{{padding:.55rem .8rem;text-align:left;border-bottom:1px solid var(--line);vertical-align:middle}}
thead th{{font-size:.8rem;text-transform:uppercase;letter-spacing:.06em;color:var(--mute)}}
tr:last-child>*{{border-bottom:0}}
.bench td{{position:relative;min-width:7rem}}
.bar{{display:block;height:.55rem;border-radius:.3rem;margin-bottom:.15rem}}
.fl{{background:var(--fl)}} .ih{{background:var(--ih)}}
.v{{font-variant-numeric:tabular-nums;font-size:.9rem}}
.verdict{{font-weight:700;white-space:nowrap}} .win .verdict{{color:var(--win)}} .lose .verdict{{color:var(--lose)}} .even .verdict{{color:var(--mute)}}
.legend{{color:var(--mute);font-size:.9rem;margin:.6rem 0}}
.legend i{{display:inline-block;width:.8rem;height:.8rem;border-radius:.2rem;vertical-align:-.1em;margin-right:.3rem}}
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(15rem,1fr));gap:1rem}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:.7rem;padding:1rem 1.1rem}}
.card h3{{margin:.1em 0 .3em;font-size:1.05rem}} .card p{{margin:0;color:var(--mute);font-size:.95rem}}
.note{{color:var(--mute);font-size:.92rem}}
footer{{margin-top:4rem;color:var(--mute);font-size:.9rem;border-top:1px solid var(--line);padding-top:1rem}}
@media(max-width:36rem){{th,td{{padding:.45rem .5rem}}.bench td{{min-width:5.5rem}}}}
</style></head><body><main>
<h1>IFHF</h1>
<p class="lede">A friendly command line for Flatpak. Install, remove and search apps by their ordinary name — no application IDs to memorise.</p>
<p class="cta"><a class="p" href="{repo}">Source on GitHub</a><a href="{repo}/blob/main/README.md">Documentation</a><a href="{repo}/blob/main/bench/RESULTS.md">Full benchmarks</a></p>
<pre><code>$ ifhf install spotify
Spotify
Online music streaming service

Application: com.spotify.Client
Remote:      flathub
Scope:       user

Install Spotify? [Y/n]</code></pre>
<p>IFHF is a thin layer. <strong>Flatpak still downloads, verifies and deploys everything</strong>; IFHF never bypasses its signatures or permissions, runs no daemon and needs no server.</p>

<h2>Why it is fast</h2>
<p>Flatpak's own <code>search</code> re-reads about 50&nbsp;MB of AppStream XML on every call. IFHF parses it once into a small local index and then answers from memory — a warm search never starts a subprocess. The first search after the index is deleted takes {fmt(cold)}; every one after that is milliseconds.</p>
<p class="legend"><i style="background:var(--fl)"></i>flatpak &nbsp; <i style="background:var(--ih)"></i>ifhf &nbsp; (shorter is faster)</p>
<table class="bench"><thead><tr><th>Task</th><th>flatpak</th><th>ifhf</th><th></th></tr></thead><tbody>
{chr(10).join(rows)}
</tbody></table>
<p class="note">Measured with <code>hyperfine</code> on a 2020 laptop (i5-10310U, <code>powersave</code> governor), Flatpak 1.18.4; the machine is noisy (σ of 5–15&nbsp;ms on fast commands). Where IFHF only wraps a Flatpak operation, it matches Flatpak or adds a little overhead for its safety checks (resolving names, “already installed”, exact partial-failure reports). Peak memory for a search: 449&nbsp;MB for Flatpak, 20&nbsp;MB for IFHF. Method and raw numbers are in the repository.</p>

<h2>Two syntaxes, one behaviour</h2>
<table><thead><tr><th>Natural</th><th>Pacman-style</th><th>Does</th></tr></thead><tbody>
<tr><td><code>ifhf install spotify discord</code></td><td><code>ifhf -S spotify discord</code></td><td>Install, one confirmation</td></tr>
<tr><td><code>ifhf remove spotify</code></td><td><code>ifhf -R spotify</code></td><td>Remove (keeps your data)</td></tr>
<tr><td><code>ifhf search browser</code></td><td><code>ifhf -Ss browser</code></td><td>Search every remote</td></tr>
<tr><td><code>ifhf info firefox</code></td><td><code>ifhf -Si firefox</code></td><td>Details and install status</td></tr>
<tr><td><code>ifhf list</code></td><td><code>ifhf -Q</code></td><td>Installed apps</td></tr>
<tr><td><code>ifhf updates</code> / <code>ifhf update</code></td><td><code>ifhf -Qu</code> / <code>ifhf -Syu</code></td><td>List / apply updates</td></tr>
<tr><td><code>ifhf sync</code></td><td><code>ifhf -Sy</code></td><td>Refresh metadata</td></tr>
</tbody></table>

<h2>Built to be safe</h2>
<div class="grid">
<div class="card"><h3>Never guesses</h3><p>Several similar matches (say, Firefox in two remotes) means you choose. In scripts it fails with a clear exit code and lists the candidates.</p></div>
<div class="card"><h3>--yes is not blind</h3><p>It never accepts a fuzzy-only match. <code>install terminal</code> will not quietly pick whatever app has “terminal” in its name.</p></div>
<div class="card"><h3>Your data is kept</h3><p>Remove leaves app data alone. <code>--delete-data</code> warns, defaults to No, and cannot be switched off in config.</p></div>
<div class="card"><h3>No shell, validated input</h3><p>Every ID and remote name is checked, and arguments reach Flatpak as separate argv entries — never through a shell.</p></div>
<div class="card"><h3>Honest failures</h3><p>Multi-app operations report exactly what succeeded and what failed. A failed metadata refresh is an error, never an empty catalogue.</p></div>
<div class="card"><h3>Tested</h3><p>Unit, integration and end-to-end tests, including the real binary against a stub Flatpak, and a real install/remove cycle in an isolated directory.</p></div>
</div>

<h2>Install</h2>
<pre><code>git clone {repo}.git
cd ifhf
cargo build --release
install -Dm755 target/release/ifhf ~/.local/bin/ifhf</code></pre>
<p class="note">Needs Rust and a working <code>flatpak</code>. Read-only commands never change your installation.</p>

<footer>IFHF is part of the IFHX family. Written in Rust. <a href="{repo}">Source</a> · <a href="{repo}/blob/main/docs/DECISIONS.md">Design decisions</a></footer>
</main></body></html>
"""
os.makedirs(os.path.join(root, "docs"), exist_ok=True)
open(os.path.join(root, "docs", "index.html"), "w").write(page)
print("wrote docs/index.html")
