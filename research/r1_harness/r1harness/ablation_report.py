"""R2.4 — ablation-table reporter (pure core).

Aggregates the R2.2b BM25 weight-sweep report and the R2.3b A/B chunker report into
a single Markdown view comparing NDCG@10 and F1@10 across (a) BM25 weight vectors and
(b) chunking strategies.  Includes a directional top-config selection.

Pure-logic core: takes *parsed* structures (VectorResult objects, aggregated A/B dicts)
and returns Markdown strings or selection dicts.  No file I/O, no binary calls here —
the thin loader and run_report.py entrypoint handle those.

Scope honesty (project_overview §7 / BRIEF-R2.4):
  - The micro-suite is a 15-query PROXY; results are DIRECTIONAL SIGNALS only.
  - This is NOT a published finding.
  - No chunker winner is asserted (that is a gated R3 determination).
"""

from __future__ import annotations

from collections import defaultdict

from .scorer import MetricAtK
from .sweep import VectorResult, rank_vectors

# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def aggregate_ab_rows(rows: list[dict], k: int = 10) -> list[dict]:
    """Aggregate per-corpus A/B rows into one dict per arm using n_queries-weighted averaging.

    ``rows`` shape (in-memory, int-keyed):
        [{"corpus_id": str, "arm": str, "n_queries": int,
          "macro_all": {int: MetricAtK}}, ...]

    Returns one dict per arm (in first-seen insertion order):
        [{"arm": str, "n_queries": int (total), "macro_all": {int: MetricAtK}}, ...]

    Weighted average per metric: Σ(nᵢ·metricᵢ) / Σ(nᵢ).
    When total n_queries == 0 for an arm, returns zeroed MetricAtK (no ZeroDivisionError).
    Empty input → [].
    """
    if not rows:
        return []

    # Determine which k values are present (use first row as reference; typically 1,5,10)
    if rows:
        sample_macro = rows[0]["macro_all"]
        k_values = sorted(sample_macro.keys())
    else:
        k_values = [k]

    # Accumulate weighted sums per arm
    arm_totals: dict[str, int] = defaultdict(int)
    # arm -> k -> accumulator dict of floats
    arm_sums: dict[str, dict[int, dict[str, float]]] = defaultdict(lambda: defaultdict(lambda: defaultdict(float)))
    # Preserve insertion order of arms
    arm_order: list[str] = []
    seen_arms: set[str] = set()

    for row in rows:
        arm = row["arm"]
        n = row["n_queries"]
        macro = row["macro_all"]

        if arm not in seen_arms:
            arm_order.append(arm)
            seen_arms.add(arm)

        arm_totals[arm] += n

        for kv, m in macro.items():
            arm_sums[arm][kv]["recall_file"] += n * m.recall_file
            arm_sums[arm][kv]["precision_file"] += n * m.precision_file
            arm_sums[arm][kv]["f1_file"] += n * m.f1_file
            arm_sums[arm][kv]["ndcg_file"] += n * m.ndcg_file
            arm_sums[arm][kv]["recall_block"] += n * m.recall_block
            arm_sums[arm][kv]["precision_block"] += n * m.precision_block
            arm_sums[arm][kv]["f1_block"] += n * m.f1_block
            arm_sums[arm][kv]["ndcg_block"] += n * m.ndcg_block

    result: list[dict] = []
    for arm in arm_order:
        total_n = arm_totals[arm]
        agg_macro: dict[int, MetricAtK] = {}
        for kv in k_values:
            sums = arm_sums[arm][kv]
            if total_n == 0:
                agg_macro[kv] = MetricAtK(
                    k=kv,
                    recall_file=0.0,
                    precision_file=0.0,
                    f1_file=0.0,
                    ndcg_file=0.0,
                    recall_block=0.0,
                    precision_block=0.0,
                    f1_block=0.0,
                    ndcg_block=0.0,
                )
            else:
                agg_macro[kv] = MetricAtK(
                    k=kv,
                    recall_file=sums["recall_file"] / total_n,
                    precision_file=sums["precision_file"] / total_n,
                    f1_file=sums["f1_file"] / total_n,
                    ndcg_file=sums["ndcg_file"] / total_n,
                    recall_block=sums["recall_block"] / total_n,
                    precision_block=sums["precision_block"] / total_n,
                    f1_block=sums["f1_block"] / total_n,
                    ndcg_block=sums["ndcg_block"] / total_n,
                )
        result.append({"arm": arm, "n_queries": total_n, "macro_all": agg_macro})

    return result


def render_markdown(
    sweep_vectors: list[VectorResult],
    aggregated_ab: list[dict],
    k: int = 10,
) -> str:
    """Render an ablation Markdown report from parsed sweep and A/B data.

    Pure: no file I/O, no binary.  Returns a Markdown string with:
      (a) A pipe-delimited weight-vector table (label, weights, F1@k, NDCG@k).
      (b) A pipe-delimited chunker arm table (arm, F1@k, NDCG@k).
      (c) A directional / PROXY / "not a published finding" disclaimer.

    Does NOT assert a chunker winner (that is a gated R3 determination, outcome-agnostic).
    """
    lines: list[str] = []

    lines.append("# R2.4 Ablation Report")
    lines.append("")
    lines.append(
        "> **Scope disclaimer:** Results are DIRECTIONAL SIGNALS from a 15-query PROXY micro-suite. "
        "This is NOT a published finding. "
        "A fuller determination requires the gated R2.5–R2.7 external-corpus run (see project_overview §7)."
    )
    lines.append("")

    # --- Section A: BM25 weight-vector table ---
    lines.append("## Section A — BM25 Weight Vectors")
    lines.append("")
    if sweep_vectors:
        lines.append(f"| Label | Weights | F1@{k} (block) | NDCG@{k} (block) |")
        lines.append("|---|---|---|---|")
        for vr in sweep_vectors:
            m = vr.macro_all[k]
            weights_str = ",".join(str(int(w)) if float(w).is_integer() else str(w) for w in vr.weights)
            flag = " ← shipped default" if vr.label == "default" else ""
            lines.append(f"| {vr.label} | {weights_str}{flag} | {m.f1_block:.4f} | {m.ndcg_block:.4f} |")
    else:
        lines.append("_(no sweep vectors provided)_")
    lines.append("")

    # --- Section B: Chunker A/B table ---
    lines.append("## Section B — Chunker A/B Comparison")
    lines.append("")
    if aggregated_ab:
        lines.append(f"| Arm | n_queries | F1@{k} (block) | NDCG@{k} (block) |")
        lines.append("|---|---|---|---|")
        for row in aggregated_ab:
            m = row["macro_all"][k]
            lines.append(f"| {row['arm']} | {row['n_queries']} | {m.f1_block:.4f} | {m.ndcg_block:.4f} |")
        lines.append("")
        lines.append(
            "_Directional only — no arm winner is asserted. Outcome-agnostic (project_overview §7); "
            "R3 gates the final determination._"
        )
    else:
        lines.append("_(no A/B rows provided)_")
    lines.append("")

    # --- Top-config note (inline summary) ---
    if sweep_vectors:
        top = select_top_config(sweep_vectors, k=k)
        best = top["best"]
        default = top["default"]
        agrees_str = (
            "YES — proxy agrees with the shipped default"
            if top["agrees"]
            else "NO — proxy favours a non-default config (directional only)"
        )
        lines.append("## Top-Config Selection (proxy directional signal)")
        lines.append("")
        lines.append(f"- Best by NDCG@{k}: **{best.label}** (NDCG={best.macro_all[k].ndcg_block:.4f})")
        lines.append(
            f"- Shipped default `(10,1,1,5,2,2,2)`: **{default.label}** (NDCG={default.macro_all[k].ndcg_block:.4f})"
        )
        lines.append(f"- Proxy agrees with default: **{agrees_str}**")
        lines.append("")
        lines.append(
            "_This is a PROXY directional signal on the micro-suite — NOT a published finding. "
            "The gated R2.5–R2.7 run over the real external corpus is required before any config change._"
        )
        lines.append("")

    return "\n".join(lines)


def select_top_config(sweep_vectors: list[VectorResult], k: int = 10) -> dict:
    """Select the best weight vector by NDCG@k and compare with the shipped default.

    Reuses ``sweep.rank_vectors`` (stable sort, ties keep grid order).

    Returns:
        {
            "best": VectorResult,    # top-ranked by NDCG@k (block)
            "default": VectorResult, # the vector whose label == "default"
            "agrees": bool,          # True iff best.label == "default"
        }
    """
    ranked = rank_vectors(sweep_vectors, k=k, key="ndcg_block")
    best = ranked[0]
    default_vec = next(vr for vr in sweep_vectors if vr.label == "default")
    return {
        "best": best,
        "default": default_vec,
        "agrees": best.label == "default",
    }
