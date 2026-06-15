"""R2.4 — ablation-table reporter entrypoint.

Loads ``runs/sweep/report.json`` (BM25 weight sweep, R2.2b) and
``runs/ab/report.json`` (A/B chunker comparison, R2.3b), renders the combined
Markdown ablation table via the pure core in ``r1harness/ablation_report.py``,
prints it, and writes ``runs/ablation/report.md``.

Does NOT assert a winner (outcome-agnostic, project_overview §7).
Directional signal on a 15-query PROXY micro-suite — NOT a published finding.

Run (from research/r1_harness/, after run_sweep.py and run_ab.py):
    python3 run_report.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

HERE = Path(__file__).resolve().parent


def _load_sweep_report(report_path: Path) -> list:
    """Load runs/sweep/report.json and reconstruct a list of VectorResult objects."""
    from dataclasses import fields

    from r1harness.scorer import MetricAtK
    from r1harness.sweep import VectorResult

    data = json.loads(report_path.read_text(encoding="utf-8"))
    results: list[VectorResult] = []
    for v in data["vectors"]:
        macro_all: dict[int, MetricAtK] = {}
        for str_k, m_dict in v["macro_all"].items():
            macro_all[int(str_k)] = MetricAtK(**{f.name: m_dict[f.name] for f in fields(MetricAtK)})
        macro_keyword: dict[int, MetricAtK] = {}
        for str_k, m_dict in v["macro_keyword"].items():
            macro_keyword[int(str_k)] = MetricAtK(**{f.name: m_dict[f.name] for f in fields(MetricAtK)})
        results.append(
            VectorResult(
                label=v["label"],
                weights=tuple(v["weights"]),
                macro_all=macro_all,
                macro_keyword=macro_keyword,
                n_queries=v["n_queries"],
                n_keyword=v["n_keyword"],
            )
        )
    return results


def _load_ab_report(report_path: Path) -> list[dict]:
    """Load runs/ab/report.json and reconstruct rows with {int: MetricAtK} macro_all."""
    from dataclasses import fields

    from r1harness.scorer import MetricAtK

    data = json.loads(report_path.read_text(encoding="utf-8"))
    rows: list[dict] = []
    for row in data["rows"]:
        macro_all: dict[int, MetricAtK] = {}
        for str_k, m_dict in row["macro_all"].items():
            macro_all[int(str_k)] = MetricAtK(**{f.name: m_dict[f.name] for f in fields(MetricAtK)})
        rows.append(
            {
                "corpus_id": row["corpus_id"],
                "arm": row["arm"],
                "n_queries": row["n_queries"],
                "macro_all": macro_all,
            }
        )
    return rows


def main() -> int:
    from r1harness.ablation_report import aggregate_ab_rows, render_markdown

    sweep_path = HERE / "runs" / "sweep" / "report.json"
    ab_path = HERE / "runs" / "ab" / "report.json"

    missing: list[str] = []
    if not sweep_path.exists():
        missing.append(f"  {sweep_path}  — run:  python3 run_sweep.py")
    if not ab_path.exists():
        missing.append(f"  {ab_path}  — run:  python3 run_ab.py")

    if missing:
        print("ERROR: required report(s) not found.  Generate them first:", file=sys.stderr)
        for msg in missing:
            print(msg, file=sys.stderr)
        return 1

    sweep_vectors = _load_sweep_report(sweep_path)
    ab_rows = _load_ab_report(ab_path)
    aggregated_ab = aggregate_ab_rows(ab_rows, k=10)

    md = render_markdown(sweep_vectors, aggregated_ab, k=10)

    print(md)

    out_dir = HERE / "runs" / "ablation"
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / "report.md"
    out_path.write_text(md, encoding="utf-8")
    print(f"\n(written to {out_path})", file=sys.stderr)
    print(
        "(Directional signal on a 15-query PROXY micro-suite — NOT a published finding; "
        "R2.5–R2.7 gate the real corpus.)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
