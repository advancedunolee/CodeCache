"""Offline R1 end-to-end validation — A0/A1/A4 on one task, no API, no cost.

Drives the full pipeline (mini-SWE-agent loop → bash/codecache actions →
trajectory logs → Layer-1/Layer-2 scoring) using mini's ``DeterministicModel``
with scripted, realistic actions. This proves the R1 plumbing end-to-end and
produces the three trajectory logs + the metrics report that R1's exit names —
a live model is a drop-in for ``DeterministicModel`` (gated on a backend choice).

Run (from research/r1_harness/, with the short-path venv that has mini-swe-agent):
    PYTHONUTF8=1 C:/ccr1/Scripts/python.exe validate_offline.py

Writes ``runs/<arm>/trajectory.jsonl`` + ``runs/report.json`` and prints a summary.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from minisweagent.models.test_models import DeterministicModel  # noqa: E402

from r1harness.arms import R1_ARMS, Task  # noqa: E402
from r1harness.codecache_tool import find_codecache_binary  # noqa: E402
from r1harness.runner import run_all  # noqa: E402

HERE = Path(__file__).resolve().parent


def deterministic_factory(outputs: list[dict]) -> DeterministicModel:
    return DeterministicModel(outputs=outputs)


def main() -> int:
    task = Task.from_dict(json.loads((HERE / "tasks" / "auth_q1.json").read_text(encoding="utf-8")))
    runs_dir = HERE / "runs"
    binary = find_codecache_binary()
    arms = [R1_ARMS["A0"], R1_ARMS["A1"], R1_ARMS["A4"]]

    report = run_all(task, arms, runs_dir, deterministic_factory, binary=binary)
    (runs_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(f"\n=== R1 offline validation — task {task.task_id!r}: {task.query!r} ===")
    print(f"gold file = {sorted(task.gold_files)}, gold block = {sorted(task.gold_blocks)}")
    print(f"binary    = {report['binary']}\n")
    print(f"{'arm':4} {'R@1 file':>9} {'R@1 blk':>8} {'F1@10 blk':>10} "
          f"{'turns→cov':>10} {'tok→cov':>9} {'tot tok':>8}  surfaced(top blocks)")
    ok = True
    for name in ("A0", "A1", "A4"):
        a = report["arms"][name]
        at1 = a["layer1"]["@1"]
        at10 = a["layer1"]["@10"]
        l2 = a["layer2"]
        top = ", ".join(f"{f}:{s}" for f, s in a["surfaced_blocks"][:2])
        print(f"{name:4} {at1['recall_file']:9.2f} {at1['recall_block']:8.2f} {at10['f1_block']:10.2f} "
              f"{str(l2['turns_to_coverage']):>10} {str(l2['tokens_to_coverage']):>9} {l2['total_tokens']:8d}  {top}")
        # plumbing assertion: every arm must surface the gold block somewhere in its trajectory
        if at10["recall_block"] < 1.0:
            ok = False
            print(f"    !! {name}: gold block NOT covered — plumbing/extractor issue")

    print(f"\nreport: {runs_dir / 'report.json'}")
    if not ok:
        print("FAIL: an arm did not cover the gold block.")
        return 1
    print("OK: all three arms ran end-to-end, logged trajectories, and covered the gold block.")
    print("    (No arm-winner claim — that is R3. This validates the apparatus only.)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
