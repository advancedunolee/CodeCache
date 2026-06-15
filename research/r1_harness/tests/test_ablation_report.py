"""RED tests for R2.4 — ablation-table reporter (pure logic; no binary, no file I/O).

Covers:
  1. Weighted A/B aggregation invariant (THE pinned test — hand-computed).
  2. F1@10 / NDCG@10 extraction from sweep VectorResult-shaped data.
  3. Markdown render — weight-vector section (pipe-delimited table).
  4. Markdown render — chunker section (one row per arm).
  5. Top-config selection (best vector by NDCG@10; default agreement check).
  6. Scope honesty (directional/PROXY disclaimer; no chunker-winner assertion).
  7. Edge — empty / zero-n_queries inputs do not raise ZeroDivisionError.

The production module ``r1harness/ablation_report.py`` does NOT exist yet — every
import here will fail with ImportError. That is the correct RED state.

Public API expected by these tests (engineering lead must implement):

    from r1harness.ablation_report import (
        aggregate_ab_rows,          # (rows: list[dict], k: int = 10) -> list[dict]
        render_markdown,            # (sweep_vectors: list[VectorResult],
                                    #  aggregated_ab: list[dict],
                                    #  k: int = 10) -> str
        select_top_config,          # (sweep_vectors: list[VectorResult],
                                    #  k: int = 10) -> dict
    )

``aggregate_ab_rows`` contract:
    - ``rows`` is the A/B report list: each dict has ``corpus_id``, ``arm``,
      ``n_queries: int``, ``macro_all: {int: MetricAtK}``.
    - Returns one dict per arm with keys ``arm``, ``n_queries`` (total), ``macro_all``
      where ``macro_all[k]`` is a MetricAtK whose metrics are n_queries-WEIGHTED averages:
      Σ(nᵢ·metricᵢ) / Σ(nᵢ).  When total n_queries == 0, returns zeroed MetricAtK
      (matching ``macro_average``'s empty-input contract).

``render_markdown`` contract:
    - Pure: structures in → Markdown string out.  No file I/O.
    - Returns a multi-section Markdown string containing:
        (a) A pipe-delimited table for weight vectors (label, weights, F1@10, NDCG@10).
        (b) A pipe-delimited table for chunker arms (arm, F1@10, NDCG@10).
        (c) A directional / PROXY disclaimer substring.
        (d) NO chunker-winner assertion.

``select_top_config`` contract:
    - Reuses ``sweep.rank_vectors`` internally (rank by ndcg_block at k).
    - Returns a dict with keys ``best``, ``default``, ``agrees``:
        - ``best``: the top-ranked VectorResult.
        - ``default``: the VectorResult whose label == "default".
        - ``agrees``: bool — True iff best.label == "default".
"""

from __future__ import annotations

import pytest

from r1harness.scorer import MetricAtK
from r1harness.sweep import VectorResult

# --- Production import (will fail RED: module does not exist yet) ---
from r1harness.ablation_report import (  # type: ignore[import]  # noqa: E402
    aggregate_ab_rows,
    render_markdown,
    select_top_config,
)

# ---------------------------------------------------------------------------
# Helpers — build MetricAtK fixtures inline (no binary, no file I/O)
# ---------------------------------------------------------------------------

_ZERO_METRIC_K10 = MetricAtK(
    k=10,
    recall_file=0.0,
    precision_file=0.0,
    f1_file=0.0,
    ndcg_file=0.0,
    recall_block=0.0,
    precision_block=0.0,
    f1_block=0.0,
    ndcg_block=0.0,
)


def _make_metric(k: int = 10, f1_block: float = 0.0, ndcg_block: float = 0.0) -> MetricAtK:
    """Build a MetricAtK with specified f1_block / ndcg_block; other fields zero."""
    return MetricAtK(
        k=k,
        recall_file=0.0,
        precision_file=0.0,
        f1_file=0.0,
        ndcg_file=0.0,
        recall_block=0.0,
        precision_block=0.0,
        f1_block=f1_block,
        ndcg_block=ndcg_block,
    )


def _make_ab_row(corpus_id: str, arm: str, n_queries: int, f1_block: float, ndcg_block: float) -> dict:
    """Build one A/B row dict (``run_ab`` / ``run_ab.py`` serialisation shape).

    ``macro_all`` keyed by *int* k (matching the in-memory shape from ``run_ab``
    before JSON serialisation).  aggregate_ab_rows works on the in-memory shape.
    """
    return {
        "corpus_id": corpus_id,
        "arm": arm,
        "n_queries": n_queries,
        "macro_all": {
            1: _make_metric(k=1),
            5: _make_metric(k=5),
            10: _make_metric(k=10, f1_block=f1_block, ndcg_block=ndcg_block),
        },
    }


def _make_vector_result(
    label: str,
    weights: tuple,
    f1_block: float,
    ndcg_block: float,
) -> VectorResult:
    """Build a VectorResult with the given F1@10 / NDCG@10 block metrics."""
    macro = {
        1: _make_metric(k=1),
        5: _make_metric(k=5),
        10: _make_metric(k=10, f1_block=f1_block, ndcg_block=ndcg_block),
    }
    return VectorResult(
        label=label,
        weights=weights,
        macro_all=macro,
        macro_keyword=macro,
        n_queries=3,
        n_keyword=2,
    )


# ---------------------------------------------------------------------------
# Scenario 1: Weighted A/B aggregation — THE pinned invariant (hand-computed)
#
# Setup:
#   arm "native":
#     corpusA  n_queries=2  f1_block@10=0.8  ndcg_block@10=0.9
#     corpusB  n_queries=3  f1_block@10=0.5  ndcg_block@10=0.4
#
# Expected weighted averages (total n = 5):
#   ndcg_block = (2*0.9 + 3*0.4) / 5 = (1.8 + 1.2) / 5 = 3.0 / 5 = 0.600
#   f1_block   = (2*0.8 + 3*0.5) / 5 = (1.6 + 1.5) / 5 = 3.1 / 5 = 0.620
#
# Plain mean (WRONG): ndcg = (0.9+0.4)/2 = 0.650 — the test deliberately rejects this.
# ---------------------------------------------------------------------------


def test_weighted_ab_aggregation_invariant():
    """n_queries-weighted aggregation equals Σ(nᵢ·metricᵢ)/Σ(nᵢ), NOT the plain mean."""
    rows = [
        _make_ab_row("corpusA", "native", n_queries=2, f1_block=0.8, ndcg_block=0.9),
        _make_ab_row("corpusB", "native", n_queries=3, f1_block=0.5, ndcg_block=0.4),
    ]

    aggregated = aggregate_ab_rows(rows, k=10)

    # Must return exactly one row (one arm: "native")
    assert len(aggregated) == 1, f"expected 1 aggregated arm row, got {len(aggregated)}"
    row = aggregated[0]
    assert row["arm"] == "native"
    assert row["n_queries"] == 5  # total across corpora

    m10 = row["macro_all"][10]

    # Hand-computed weighted averages — use pytest.approx for float equality
    assert m10.ndcg_block == pytest.approx(0.600, abs=1e-9), (
        f"Expected weighted ndcg_block=0.600, got {m10.ndcg_block:.6f}. "
        f"Plain mean would be 0.650 — weighted must differ."
    )
    assert m10.f1_block == pytest.approx(0.620, abs=1e-9), f"Expected weighted f1_block=0.620, got {m10.f1_block:.6f}."


def test_weighted_ab_aggregation_two_arms():
    """Both arms aggregated correctly when two arms are present across two corpora."""
    rows = [
        _make_ab_row("corpusA", "native", n_queries=2, f1_block=0.8, ndcg_block=0.9),
        _make_ab_row("corpusA", "stub", n_queries=2, f1_block=0.6, ndcg_block=0.7),
        _make_ab_row("corpusB", "native", n_queries=3, f1_block=0.5, ndcg_block=0.4),
        _make_ab_row("corpusB", "stub", n_queries=3, f1_block=0.3, ndcg_block=0.2),
    ]

    aggregated = aggregate_ab_rows(rows, k=10)

    assert len(aggregated) == 2, f"expected 2 arm rows (native + stub), got {len(aggregated)}"
    by_arm = {r["arm"]: r for r in aggregated}
    assert "native" in by_arm
    assert "stub" in by_arm

    # native: (2*0.9 + 3*0.4)/5 = 0.600
    assert by_arm["native"]["macro_all"][10].ndcg_block == pytest.approx(0.600, abs=1e-9)
    # stub: (2*0.7 + 3*0.2)/5 = (1.4+0.6)/5 = 2.0/5 = 0.400
    assert by_arm["stub"]["macro_all"][10].ndcg_block == pytest.approx(0.400, abs=1e-9)


# ---------------------------------------------------------------------------
# Scenario 2: F1@10 / NDCG@10 extraction from sweep VectorResult-shaped data
# ---------------------------------------------------------------------------


def test_f1_ndcg_extraction_from_vector_results():
    """render_markdown extracts F1@10 and NDCG@10 at block granularity from VectorResult."""
    vectors = [
        _make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.75, ndcg_block=0.80),
        _make_vector_result("flat", (1, 1, 1, 1, 1, 1, 1), f1_block=0.50, ndcg_block=0.55),
    ]
    aggregated_ab: list[dict] = []  # empty A/B section OK for this test

    md = render_markdown(vectors, aggregated_ab, k=10)

    # Both vector labels present
    assert "default" in md
    assert "flat" in md
    # Both numeric values present (at least to 2 decimal places)
    assert "0.75" in md or "0.750" in md
    assert "0.80" in md or "0.800" in md


# ---------------------------------------------------------------------------
# Scenario 3: Markdown render — weight-vector section structural markers
# ---------------------------------------------------------------------------


def test_markdown_render_weight_vector_table_structure():
    """Sweep table is pipe-delimited with header + separator + one row per vector."""
    vectors = [
        _make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.75, ndcg_block=0.80),
        _make_vector_result("flat", (1, 1, 1, 1, 1, 1, 1), f1_block=0.50, ndcg_block=0.55),
        _make_vector_result("name_only", (10, 0, 0, 0, 0, 0, 0), f1_block=0.60, ndcg_block=0.65),
    ]
    aggregated_ab: list[dict] = []

    md = render_markdown(vectors, aggregated_ab, k=10)

    # Pipe-delimited table markers
    assert "|" in md, "Markdown table must use pipe delimiters"
    assert "---" in md, "Markdown table must have a separator row (---)"

    # Each vector label must appear in the output
    for v in vectors:
        assert v.label in md, f"Vector label '{v.label}' missing from rendered Markdown"

    # F1@10 and NDCG@10 column headers (case-insensitive check)
    md_lower = md.lower()
    assert "f1" in md_lower, "Weight-vector table must include F1 column"
    assert "ndcg" in md_lower, "Weight-vector table must include NDCG column"


# ---------------------------------------------------------------------------
# Scenario 4: Markdown render — chunker section
# ---------------------------------------------------------------------------


def test_markdown_render_chunker_section():
    """Chunker table has one row per arm (native, stub) with F1@10 and NDCG@10."""
    vectors: list[VectorResult] = []  # empty sweep section OK for this test
    aggregated_ab = [
        {
            "arm": "native",
            "n_queries": 5,
            "macro_all": {10: _make_metric(k=10, f1_block=0.72, ndcg_block=0.78)},
        },
        {
            "arm": "stub",
            "n_queries": 5,
            "macro_all": {10: _make_metric(k=10, f1_block=0.65, ndcg_block=0.70)},
        },
    ]

    md = render_markdown(vectors, aggregated_ab, k=10)

    # Both arm labels present
    assert "native" in md
    assert "stub" in md

    # Pipe-delimited table markers
    assert "|" in md
    assert "---" in md

    # Numeric values present
    assert "0.72" in md or "0.720" in md
    assert "0.78" in md or "0.780" in md


# ---------------------------------------------------------------------------
# Scenario 5: Top-config selection
# ---------------------------------------------------------------------------


def test_select_top_config_non_default_wins():
    """When a non-default vector has higher NDCG@10, agrees == False."""
    default_vec = _make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.70, ndcg_block=0.75)
    winner_vec = _make_vector_result("name_strong", (20, 1, 1, 5, 2, 2, 2), f1_block=0.80, ndcg_block=0.85)
    loser_vec = _make_vector_result("flat", (1, 1, 1, 1, 1, 1, 1), f1_block=0.40, ndcg_block=0.45)

    vectors = [default_vec, winner_vec, loser_vec]  # not pre-sorted

    result = select_top_config(vectors, k=10)

    assert result["best"].label == "name_strong", (
        f"Expected 'name_strong' (ndcg_block=0.85) to win, got '{result['best'].label}'"
    )
    assert result["default"].label == "default"
    assert result["agrees"] is False, "When a non-default vector wins, agrees must be False"


def test_select_top_config_default_wins():
    """When the default vector has the highest NDCG@10, agrees == True."""
    default_vec = _make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.90, ndcg_block=0.95)
    other_vec = _make_vector_result("flat", (1, 1, 1, 1, 1, 1, 1), f1_block=0.60, ndcg_block=0.65)

    vectors = [other_vec, default_vec]  # default is NOT first in grid order

    result = select_top_config(vectors, k=10)

    assert result["best"].label == "default"
    assert result["default"].label == "default"
    assert result["agrees"] is True, "When default wins, agrees must be True"


def test_select_top_config_surfaces_default_weights():
    """select_top_config always surfaces the shipped default (10,1,1,5,2,2,2)."""
    default_vec = _make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.80, ndcg_block=0.85)
    other_vec = _make_vector_result("name_only", (10, 0, 0, 0, 0, 0, 0), f1_block=0.50, ndcg_block=0.55)

    result = select_top_config([default_vec, other_vec], k=10)

    # The default VectorResult is always returned regardless of ranking
    assert result["default"] is default_vec
    assert tuple(result["default"].weights) == (10, 1, 1, 5, 2, 2, 2)


# ---------------------------------------------------------------------------
# Scenario 6: Scope honesty — directional/PROXY disclaimer; no winner claim
# ---------------------------------------------------------------------------


def test_scope_honesty_disclaimer_present():
    """Rendered report contains a directional/PROXY disclaimer."""
    vectors = [_make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.75, ndcg_block=0.80)]
    aggregated_ab = [
        {
            "arm": "native",
            "n_queries": 5,
            "macro_all": {10: _make_metric(k=10, f1_block=0.70, ndcg_block=0.75)},
        }
    ]

    md = render_markdown(vectors, aggregated_ab, k=10)
    md_lower = md.lower()

    # Must contain directional-proxy hedging
    assert "directional" in md_lower, "Rendered report must include 'directional' disclaimer (scope honesty, R2 §7)"
    assert "proxy" in md_lower, "Rendered report must include 'proxy' disclaimer (15-query micro-suite is a PROXY)"
    assert "not a published finding" in md_lower or "not a finding" in md_lower, (
        "Rendered report must disclaim that results are not a published finding"
    )


def test_scope_honesty_no_winner_claim():
    """Rendered report does NOT assert a chunker winner (native vs stub)."""
    vectors = [_make_vector_result("default", (10, 1, 1, 5, 2, 2, 2), f1_block=0.75, ndcg_block=0.80)]
    # native clearly "wins" numerically — but report must not claim so
    aggregated_ab = [
        {
            "arm": "native",
            "n_queries": 5,
            "macro_all": {10: _make_metric(k=10, f1_block=0.90, ndcg_block=0.95)},
        },
        {
            "arm": "stub",
            "n_queries": 5,
            "macro_all": {10: _make_metric(k=10, f1_block=0.40, ndcg_block=0.45)},
        },
    ]

    md = render_markdown(vectors, aggregated_ab, k=10)
    md_lower = md.lower()

    # Must NOT contain a decisive winner verdict (e.g. "native wins", "stub wins")
    # Check for common "winner" phrasing patterns
    forbidden_phrases = [
        "native wins",
        "stub wins",
        "winner: native",
        "winner: stub",
        "native is better",
        "stub is better",
    ]
    for phrase in forbidden_phrases:
        assert phrase not in md_lower, (
            f"Rendered report must not assert a chunker winner; found forbidden phrase: '{phrase}'"
        )


# ---------------------------------------------------------------------------
# Scenario 7: Edge — empty / zero inputs do not divide-by-zero
# ---------------------------------------------------------------------------


def test_aggregate_empty_rows_returns_empty():
    """aggregate_ab_rows over an empty list returns an empty list (no crash)."""
    result = aggregate_ab_rows([], k=10)
    assert result == [], f"Expected [] for empty input, got {result!r}"


def test_aggregate_zero_n_queries_returns_zeroed_metrics():
    """A row with n_queries=0 does not cause ZeroDivisionError; returns zeroed MetricAtK."""
    rows = [
        _make_ab_row("corpusA", "native", n_queries=0, f1_block=0.0, ndcg_block=0.0),
    ]

    # Must not raise
    result = aggregate_ab_rows(rows, k=10)

    assert len(result) == 1
    assert result[0]["n_queries"] == 0
    m10 = result[0]["macro_all"][10]
    # All metrics must be zero (nothing to aggregate)
    assert m10.ndcg_block == 0.0
    assert m10.f1_block == 0.0


def test_aggregate_mixed_zero_and_nonzero_n_queries():
    """A corpus with n_queries=0 is correctly excluded from the weighted sum."""
    rows = [
        _make_ab_row("corpusA", "native", n_queries=0, f1_block=0.9, ndcg_block=0.9),
        _make_ab_row("corpusB", "native", n_queries=4, f1_block=0.6, ndcg_block=0.5),
    ]

    result = aggregate_ab_rows(rows, k=10)

    assert len(result) == 1
    m10 = result[0]["macro_all"][10]
    # Only corpusB contributes (n=4); corpusA is zero-weight
    # weighted ndcg = (0*0.9 + 4*0.5)/4 = 0.5
    assert m10.ndcg_block == pytest.approx(0.5, abs=1e-9)
    assert result[0]["n_queries"] == 4


def test_render_markdown_empty_sweep_and_empty_ab():
    """render_markdown with both inputs empty does not raise ZeroDivisionError."""
    md = render_markdown([], [], k=10)
    # Must return a string (even if mostly empty/placeholder)
    assert isinstance(md, str)
    # Must still carry the disclaimer
    md_lower = md.lower()
    assert "directional" in md_lower or "proxy" in md_lower
