#!/usr/bin/env python3
"""Generate an HTML report from cargo-mutants outcomes.json."""

import json
import sys
from pathlib import Path
from datetime import datetime, timezone


def load_diff(mutants_out: Path, diff_path: str | None) -> str:
    if not diff_path:
        return ""
    p = mutants_out / diff_path
    return p.read_text() if p.exists() else ""


def fmt_duration(seconds: float) -> str:
    return f"{seconds:.1f}s"


def status_class(summary: str) -> str:
    return {
        "MissedMutant": "missed",
        "CaughtMutant": "caught",
        "Unviable": "unviable",
        "Timeout": "timeout",
        "Success": "success",
    }.get(summary, "unknown")


def status_label(summary: str) -> str:
    return {
        "MissedMutant": "MISSED",
        "CaughtMutant": "CAUGHT",
        "Unviable": "UNVIABLE",
        "Timeout": "TIMEOUT",
        "Success": "BASELINE",
    }.get(summary, summary)


def render_mutant_row(outcome: dict, mutants_out: Path, idx: int) -> str:
    summary = outcome["summary"]
    scenario = outcome["scenario"]
    diff_content = load_diff(mutants_out, outcome.get("diff_path"))

    if isinstance(scenario, dict) and "Mutant" in scenario:
        m = scenario["Mutant"]
        file_ = m["file"]
        line = m["span"]["start"]["line"]
        col = m["span"]["start"]["column"]
        fn_name = m["function"]["function_name"]
        ret = m["function"].get("return_type", "")
        replacement = m["replacement"]
        genre = m["genre"]
        location = f"{file_}:{line}:{col}"
        label = f"<code>{fn_name}</code> {ret}"
        repl_html = f"<code class='replacement'>{_esc(replacement)}</code>"
        genre_badge = f"<span class='badge genre'>{_esc(genre)}</span>"
    else:
        location = "Baseline"
        label = "Baseline build+test"
        repl_html = ""
        genre_badge = ""

    total_dur = sum(r["duration"] for r in outcome.get("phase_results", []))
    dur_str = fmt_duration(total_dur)
    cls = status_class(summary)
    lbl = status_label(summary)

    diff_id = f"diff-{idx}"
    diff_section = ""
    if diff_content:
        diff_section = f"""
        <div class='diff-toggle' onclick="toggle('{diff_id}')">▶ show diff</div>
        <pre id='{diff_id}' class='diff hidden'>{_esc(diff_content)}</pre>"""

    return f"""
    <tr class='row-{cls}' onclick="this.classList.toggle('expanded')">
      <td><span class='status {cls}'>{lbl}</span></td>
      <td class='loc'>{_esc(location)}</td>
      <td>{label} {genre_badge}</td>
      <td>{repl_html}</td>
      <td class='dur'>{dur_str}</td>
    </tr>
    {f"<tr class='diff-row'><td colspan='5'>{diff_section}</td></tr>" if diff_section else ""}"""


def _esc(s: str) -> str:
    return (s
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;"))


def generate(outcomes_path: Path, output_path: Path) -> None:
    data = json.loads(outcomes_path.read_text())
    mutants_out = outcomes_path.parent

    total = data["total_mutants"]
    missed = data["missed"]
    caught = data["caught"]
    timeout = data.get("timeout", 0)
    unviable = data.get("unviable", 0)
    version = data.get("cargo_mutants_version", "?")
    start = data.get("start_time", "")
    end = data.get("end_time", "")
    score = round(caught / (caught + missed) * 100, 1) if (caught + missed) > 0 else 0

    score_class = "good" if score >= 80 else "warn" if score >= 60 else "bad"

    rows_missed = []
    rows_caught = []
    rows_other = []

    for idx, outcome in enumerate(data["outcomes"]):
        s = outcome["summary"]
        row = render_mutant_row(outcome, mutants_out, idx)
        if s == "MissedMutant":
            rows_missed.append(row)
        elif s == "CaughtMutant":
            rows_caught.append(row)
        else:
            rows_other.append(row)

    def table(rows: list[str], tab_id: str) -> str:
        if not rows:
            return "<p class='empty'>None.</p>"
        return f"""
        <table id='{tab_id}'>
          <thead><tr>
            <th>Status</th><th>Location</th><th>Function</th>
            <th>Replacement</th><th>Time</th>
          </tr></thead>
          <tbody>{''.join(rows)}</tbody>
        </table>"""

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Basilisk Mutation Report</title>
<style>
  :root {{
    --bg: #0d1117; --surface: #161b22; --border: #30363d;
    --text: #e6edf3; --muted: #8b949e;
    --red: #f85149; --green: #3fb950; --yellow: #d29922; --blue: #58a6ff;
    --missed-bg: #2d1b1b; --caught-bg: #0d2016; --other-bg: #1c1c2e;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: var(--bg); color: var(--text); font: 14px/1.5 'JetBrains Mono', 'Fira Code', monospace; }}
  header {{ padding: 2rem; border-bottom: 1px solid var(--border); }}
  h1 {{ font-size: 1.5rem; color: var(--blue); }}
  h1 span {{ color: var(--muted); font-size: 1rem; font-weight: normal; margin-left: 1rem; }}
  .meta {{ color: var(--muted); font-size: 0.8rem; margin-top: 0.5rem; }}
  .stats {{ display: flex; gap: 1.5rem; padding: 1.5rem 2rem; border-bottom: 1px solid var(--border); flex-wrap: wrap; }}
  .stat {{ background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 1rem 1.5rem; min-width: 140px; }}
  .stat .val {{ font-size: 2rem; font-weight: bold; }}
  .stat .lbl {{ color: var(--muted); font-size: 0.75rem; text-transform: uppercase; letter-spacing: .05em; }}
  .score.good .val {{ color: var(--green); }}
  .score.warn .val {{ color: var(--yellow); }}
  .score.bad  .val {{ color: var(--red); }}
  .val.missed {{ color: var(--red); }}
  .val.caught {{ color: var(--green); }}
  .val.timeout {{ color: var(--yellow); }}
  .tabs {{ display: flex; gap: 0; border-bottom: 1px solid var(--border); padding: 0 2rem; }}
  .tab {{ padding: 0.75rem 1.25rem; cursor: pointer; border-bottom: 2px solid transparent; color: var(--muted); transition: all .15s; }}
  .tab:hover {{ color: var(--text); }}
  .tab.active {{ color: var(--blue); border-bottom-color: var(--blue); }}
  .tab-content {{ display: none; padding: 1.5rem 2rem; }}
  .tab-content.active {{ display: block; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
  th {{ text-align: left; padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border); color: var(--muted); font-weight: normal; text-transform: uppercase; font-size: 0.7rem; letter-spacing: .05em; }}
  td {{ padding: 0.4rem 0.75rem; border-bottom: 1px solid var(--border); vertical-align: top; }}
  tr.row-missed {{ background: var(--missed-bg); }}
  tr.row-caught {{ background: var(--caught-bg); }}
  tr.row-unviable, tr.row-timeout {{ background: var(--other-bg); }}
  tr:hover td {{ filter: brightness(1.2); }}
  .status {{ display: inline-block; padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.7rem; font-weight: bold; letter-spacing: .05em; }}
  .status.missed {{ background: #5c1a1a; color: var(--red); }}
  .status.caught {{ background: #0f2d1a; color: var(--green); }}
  .status.unviable {{ background: #1c1c3a; color: #8888cc; }}
  .status.timeout {{ background: #2d2200; color: var(--yellow); }}
  .status.success {{ background: #1a2a3a; color: var(--blue); }}
  .loc {{ color: var(--muted); font-size: 0.8rem; white-space: nowrap; }}
  .replacement {{ background: #1c2a1c; color: var(--green); padding: 0 0.3rem; border-radius: 3px; }}
  .dur {{ color: var(--muted); text-align: right; white-space: nowrap; }}
  .badge {{ display: inline-block; padding: 0.1rem 0.4rem; border-radius: 3px; font-size: 0.65rem; background: #1c2233; color: #7aa2d4; margin-left: 0.3rem; }}
  .diff-toggle {{ color: var(--blue); cursor: pointer; font-size: 0.75rem; padding: 0.3rem 0; }}
  .diff-toggle:hover {{ text-decoration: underline; }}
  .diff {{ background: #0a0f16; border: 1px solid var(--border); border-radius: 4px; padding: 0.75rem; font-size: 0.75rem; overflow-x: auto; white-space: pre; margin-top: 0.5rem; }}
  .diff.hidden {{ display: none; }}
  .empty {{ color: var(--muted); padding: 1rem 0; }}
  .filter {{ margin-bottom: 1rem; }}
  .filter input {{ background: var(--surface); border: 1px solid var(--border); color: var(--text); padding: 0.4rem 0.75rem; border-radius: 6px; font-size: 0.85rem; width: 320px; font-family: inherit; }}
  .filter input:focus {{ outline: none; border-color: var(--blue); }}
</style>
</head>
<body>
<header>
  <h1>Basilisk Mutation Report <span>cargo-mutants v{_esc(version)}</span></h1>
  <div class="meta">{_esc(start)} → {_esc(end)}</div>
</header>

<div class="stats">
  <div class="stat score {score_class}">
    <div class="val">{score}%</div>
    <div class="lbl">Mutation Score</div>
  </div>
  <div class="stat">
    <div class="val">{total}</div>
    <div class="lbl">Total Mutants</div>
  </div>
  <div class="stat">
    <div class="val missed">{missed}</div>
    <div class="lbl">Missed</div>
  </div>
  <div class="stat">
    <div class="val caught">{caught}</div>
    <div class="lbl">Caught</div>
  </div>
  <div class="stat">
    <div class="val timeout">{timeout}</div>
    <div class="lbl">Timeout</div>
  </div>
  <div class="stat">
    <div class="val" style="color:var(--muted)">{unviable}</div>
    <div class="lbl">Unviable</div>
  </div>
</div>

<div class="tabs">
  <div class="tab active" onclick="switchTab('missed')">Missed ({missed})</div>
  <div class="tab" onclick="switchTab('caught')">Caught ({caught})</div>
  <div class="tab" onclick="switchTab('other')">Other ({len(rows_other)})</div>
</div>

<div id="tab-missed" class="tab-content active">
  <div class="filter"><input type="text" placeholder="Filter by file, function, replacement…" oninput="filterTable('tab-missed-table', this.value)"></div>
  {table(rows_missed, 'tab-missed-table')}
</div>
<div id="tab-caught" class="tab-content">
  <div class="filter"><input type="text" placeholder="Filter by file, function, replacement…" oninput="filterTable('tab-caught-table', this.value)"></div>
  {table(rows_caught, 'tab-caught-table')}
</div>
<div id="tab-other" class="tab-content">
  {table(rows_other, 'tab-other-table')}
</div>

<script>
function switchTab(name) {{
  document.querySelectorAll('.tab').forEach((t, i) => {{
    const names = ['missed','caught','other'];
    t.classList.toggle('active', names[i] === name);
  }});
  document.querySelectorAll('.tab-content').forEach(c => {{
    c.classList.toggle('active', c.id === 'tab-' + name);
  }});
}}
function toggle(id) {{
  const el = document.getElementById(id);
  const btn = el.previousElementSibling;
  el.classList.toggle('hidden');
  btn.textContent = el.classList.contains('hidden') ? '▶ show diff' : '▼ hide diff';
}}
function filterTable(tableId, query) {{
  const q = query.toLowerCase();
  const rows = document.querySelectorAll('#' + tableId + ' tbody tr');
  rows.forEach(row => {{
    row.style.display = row.textContent.toLowerCase().includes(q) ? '' : 'none';
  }});
}}
</script>
</body>
</html>"""

    output_path.write_text(html)
    print(f"Report written to {output_path}")


if __name__ == "__main__":
    outcomes = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("mutants.out/outcomes.json")
    output = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("mutants_report.html")
    generate(outcomes, output)
