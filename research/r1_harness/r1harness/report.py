"""Score a trajectory into the Layer-1 / Layer-2 metrics report.

Pure: depends only on :mod:`r1harness.scorer` / :mod:`r1harness.trajectory` /
:mod:`r1harness.arms` — **not** on mini-SWE-agent — so scoring is importable and
testable under any interpreter (the agent *runner* needs mini; scoring does not).
"""

from __future__ import annotations

from dataclasses import asdict
from pathlib import Path

from . import scorer
from .arms import Task
from .trajectory import (
    read_trajectory,
    surfaced_lists,
    tokens_to_coverage,
    total_tokens,
    turns_to_coverage,
)


def score_trajectory(traj_path: Path, task: Task) -> dict:
    """Compute the Layer-1 + Layer-2 metrics for one arm's trajectory."""
    _, turns = read_trajectory(traj_path)
    files, blocks = surfaced_lists(turns)
    # Across turns the same item can surface repeatedly (e.g. grep then cat both show a symbol).
    # The "retrieved set" is membership-based, so de-duplicate (keeping earliest/best rank) before
    # scoring — otherwise a repeated gold hit would inflate recall above 1.0. The scorer itself stays
    # a verbatim port of retrieval_quality.rs (which never saw dups: one query → unique blocks).
    metrics = scorer.score_query(
        scorer.dedup_first(files), scorer.dedup_first(blocks), task.gold_files, task.gold_blocks
    )
    return {
        "layer1": {f"@{m.k}": asdict(m) for m in metrics},
        "layer2": {
            "total_tokens": total_tokens(turns),
            "turns_to_coverage": turns_to_coverage(turns, task.gold_blocks),
            "tokens_to_coverage": tokens_to_coverage(turns, task.gold_blocks),
            "n_turns": len(turns),
        },
        "surfaced_files": scorer.dedup_first(files),
        "surfaced_blocks": list(dict.fromkeys(blocks)),
    }
