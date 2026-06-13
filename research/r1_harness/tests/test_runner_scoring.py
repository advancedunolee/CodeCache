"""Regression test for trajectory scoring (no mini-swe-agent / no binary needed).

Guards the de-duplication fix: a gold block that surfaces on multiple turns must
NOT push recall above 1.0. ``score_trajectory`` imports only scorer/trajectory/
arms, so this runs under the base interpreter.
"""

from r1harness.arms import Task
from r1harness.report import score_trajectory
from r1harness.trajectory import TrajectoryLogger, TrajectoryMeta


def _task() -> Task:
    return Task(
        task_id="auth_q1",
        corpus_id="auth_module",
        query="authenticate user credentials",
        query_type="keyword",
        gold_files={"src/auth/authenticate.py"},
        gold_blocks={("src/auth/authenticate.py", "authenticate_user")},
    )


def test_repeated_gold_block_does_not_inflate_recall(tmp_path):
    path = tmp_path / "traj.jsonl"
    meta = TrajectoryMeta("A0", "auth_q1", "deterministic", 0.0, "auth_module", "authenticate user credentials")
    logger = TrajectoryLogger(path, meta)
    # grep turn surfaces the gold block; cat turn surfaces it AGAIN plus a sibling.
    logger.log_turn("grep -rn def src", "bash", "...",
                    files_surfaced=["src/auth/authenticate.py"],
                    blocks_surfaced=[("src/auth/authenticate.py", "authenticate_user")])
    logger.log_turn("cat src/auth/authenticate.py", "bash", "...",
                    files_surfaced=["src/auth/authenticate.py"],
                    blocks_surfaced=[("src/auth/authenticate.py", "authenticate_user"),
                                     ("src/auth/authenticate.py", "verify_password")])

    result = score_trajectory(path, _task())
    at10 = result["layer1"]["@10"]
    # recall must be exactly 1.0 (one gold block, found) — never 2.0
    assert at10["recall_block"] == 1.0
    assert 0.0 <= at10["precision_block"] <= 1.0
    # two distinct blocks surfaced, one is gold → precision@10 = 1/2 = 0.5
    assert abs(at10["precision_block"] - 0.5) < 1e-9
    assert result["layer2"]["turns_to_coverage"] == 1
