#!/usr/bin/env python3
"""Generate a cargo-mutants HTML report and enforce the score ratchet."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


SCORE_FILE_VERSION = 1


@dataclass(frozen=True)
class MutationScore:
    date: str
    total: int
    caught: int
    missed: int
    timeout: int
    unviable: int
    kill_rate: float

    @classmethod
    def from_counts(cls, counts: dict[str, int], date: str) -> "MutationScore":
        return cls(
            date=date,
            total=counts["total"],
            caught=counts["caught"],
            missed=counts["missed"],
            timeout=counts["timeout"],
            unviable=counts["unviable"],
            kill_rate=score_percentage(counts),
        )

    @classmethod
    def from_json(cls, raw: dict[str, Any]) -> "MutationScore":
        return cls(
            date=str(raw["date"]),
            total=int(raw["total"]),
            caught=int(raw["caught"]),
            missed=int(raw["missed"]),
            timeout=int(raw["timeout"]),
            unviable=int(raw["unviable"]),
            kill_rate=float(raw["kill_rate"]),
        )

    def to_json(self) -> dict[str, int | float | str]:
        return asdict(self)


class MutationScoreRegression(Exception):
    def __init__(self, regressions: list[str]) -> None:
        super().__init__("Mutation score regression detected")
        self.regressions = regressions


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
    return (
        s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def load_outcomes(outcomes_path: Path) -> dict[str, Any]:
    return json.loads(outcomes_path.read_text(encoding="utf-8"))


def is_mutant_outcome(outcome: dict[str, Any]) -> bool:
    scenario = outcome.get("scenario")
    return isinstance(scenario, dict) and "Mutant" in scenario


def tally_outcomes(data: dict[str, Any]) -> dict[str, int]:
    counts = {"caught": 0, "missed": 0, "timeout": 0, "unviable": 0, "total": 0}
    for outcome in data.get("outcomes", []):
        if not is_mutant_outcome(outcome):
            continue
        counts["total"] += 1
        match outcome.get("summary"):
            case "CaughtMutant":
                counts["caught"] += 1
            case "MissedMutant":
                counts["missed"] += 1
            case "Timeout":
                counts["timeout"] += 1
            case "Unviable":
                counts["unviable"] += 1
    return counts


def mutation_counts(data: dict[str, Any]) -> dict[str, int]:
    if "total_mutants" not in data:
        return tally_outcomes(data)
    return {
        "total": int(data["total_mutants"]),
        "caught": int(data["caught"]),
        "missed": int(data["missed"]),
        "timeout": int(data.get("timeout", 0)),
        "unviable": int(data.get("unviable", 0)),
    }


def score_percentage(counts: dict[str, int]) -> float:
    viable = counts["caught"] + counts["missed"] + counts["timeout"]
    return round(100.0 * counts["caught"] / viable, 2) if viable > 0 else 0.0


def load_score_book(scores_path: Path) -> dict[str, Any]:
    if scores_path.suffix != ".json":
        raise ValueError(f"mutation score baseline must be JSON: {scores_path}")
    if not scores_path.exists():
        return {"version": SCORE_FILE_VERSION, "scores": {}}
    raw = json.loads(scores_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict) or not isinstance(raw.get("scores"), dict):
        raise ValueError(f"invalid mutation score baseline: {scores_path}")
    return raw


def write_score_book(scores_path: Path, score_book: dict[str, Any]) -> None:
    scores_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = scores_path.with_suffix(f"{scores_path.suffix}.tmp")
    content = json.dumps(score_book, indent=2, sort_keys=True) + "\n"
    tmp_path.write_text(content, encoding="utf-8")
    tmp_path.replace(scores_path)


def baseline_for_scope(score_book: dict[str, Any], scope: str) -> MutationScore | None:
    raw_score = score_book["scores"].get(scope)
    if raw_score is None:
        return None
    if not isinstance(raw_score, dict):
        raise ValueError(f"invalid mutation score entry for scope={scope!r}")
    return MutationScore.from_json(raw_score)


def regression_messages(fresh: MutationScore, baseline: MutationScore) -> list[str]:
    regressions: list[str] = []
    if fresh.caught < baseline.caught:
        regressions.append(f"caught dropped {baseline.caught} -> {fresh.caught}")
    if fresh.missed > baseline.missed:
        regressions.append(f"missed increased {baseline.missed} -> {fresh.missed}")
    if fresh.timeout > baseline.timeout:
        regressions.append(f"timeout increased {baseline.timeout} -> {fresh.timeout}")
    if fresh.unviable > baseline.unviable:
        regressions.append(
            f"unviable increased {baseline.unviable} -> {fresh.unviable}"
        )
    if fresh.kill_rate < baseline.kill_rate:
        regressions.append(
            f"kill_rate dropped {baseline.kill_rate}% -> {fresh.kill_rate}%"
        )
    return regressions


def score_summary(score: MutationScore) -> str:
    return (
        f"total={score.total} caught={score.caught} missed={score.missed} "
        f"timeout={score.timeout} unviable={score.unviable} "
        f"kill_rate={score.kill_rate}%"
    )


def record_score(data: dict[str, Any], scores_path: Path, scope: str) -> MutationScore:
    score_book = load_score_book(scores_path)
    fresh = MutationScore.from_counts(
        mutation_counts(data), dt.date.today().isoformat()
    )
    baseline = baseline_for_scope(score_book, scope)
    print(f"Fresh mutation score ({scope}): {score_summary(fresh)}", file=sys.stderr)
    if baseline is not None:
        print(
            f"Baseline ({baseline.date}, {scope}): {score_summary(baseline)}",
            file=sys.stderr,
        )
        regressions = regression_messages(fresh, baseline)
        if regressions:
            raise MutationScoreRegression(regressions)
    score_book["version"] = SCORE_FILE_VERSION
    score_book["scores"][scope] = fresh.to_json()
    write_score_book(scores_path, score_book)
    print(f"Mutation score baseline overwritten: {scores_path}", file=sys.stderr)
    return fresh


def emit_regression_error(error: MutationScoreRegression) -> None:
    print("Mutation score regression detected:", file=sys.stderr)
    for regression in error.regressions:
        print(f"  - {regression}", file=sys.stderr)
    print(
        "Baseline was not overwritten. Add tests until the mutation score "
        "holds or improves.",
        file=sys.stderr,
    )


def generate_from_data(
    data: dict[str, Any], mutants_out: Path, output_path: Path
) -> None:
    total = data["total_mutants"]
    missed = data["missed"]
    caught = data["caught"]
    timeout = data.get("timeout", 0)
    unviable = data.get("unviable", 0)
    version = data.get("cargo_mutants_version", "?")
    start = data.get("start_time", "")
    end = data.get("end_time", "")
    score = round(score_percentage(mutation_counts(data)), 1)

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
          <tbody>{"".join(rows)}</tbody>
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
  {table(rows_missed, "tab-missed-table")}
</div>
<div id="tab-caught" class="tab-content">
  <div class="filter"><input type="text" placeholder="Filter by file, function, replacement…" oninput="filterTable('tab-caught-table', this.value)"></div>
  {table(rows_caught, "tab-caught-table")}
</div>
<div id="tab-other" class="tab-content">
  {table(rows_other, "tab-other-table")}
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

    output_path.write_text(html, encoding="utf-8")
    print(f"Report written to {output_path}")


def generate(outcomes_path: Path, output_path: Path) -> None:
    data = load_outcomes(outcomes_path)
    generate_from_data(data, outcomes_path.parent, output_path)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "outcomes", nargs="?", type=Path, default=Path("mutants.out/outcomes.json")
    )
    parser.add_argument(
        "output", nargs="?", type=Path, default=Path("mutants_report.html")
    )
    parser.add_argument("--scores", type=Path, help="JSON baseline to ratchet")
    parser.add_argument("--scope", default="working")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    data = load_outcomes(args.outcomes)
    generate_from_data(data, args.outcomes.parent, args.output)
    if args.scores is None:
        return 0
    try:
        record_score(data, args.scores, args.scope)
    except MutationScoreRegression as error:
        emit_regression_error(error)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
