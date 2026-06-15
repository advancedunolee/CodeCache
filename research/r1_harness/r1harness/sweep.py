"""R2.2b — BM25 per-column weight sweep over the shared micro-suite (Layer-1).

Pure-logic core of the weight ablation (D23 axis): given the shared gold suite
(``tests/fixtures/retrieval_quality/micro_suite.json``) and a grid of per-column ``bm25()``
weight vectors (R2.2a / D24), score each vector's retrieval with the M10.2 + NDCG@10 scorer and
macro-average across the 15 queries into one ablation row per vector.

Retrieval is injected as ``query_fn`` so this module stays binary-free and unit-testable;
``run_sweep.py`` supplies the real one (materialise -> index -> ``codecache query --bm25-weights``).
The shipped default ``10,1,1,5,2,2,2`` is the grid's baseline row; the polished ablation TABLE is
R2.4 (this emits the raw per-vector rows).

Scope honesty: the micro-suite is a 15-query PROXY (its own description says "R2 swaps in the real
ContextBench corpus using the identical scorer"), so a sweep here validates the apparatus + gives a
directional signal — it is NOT a published weights finding (that is the gated R2.5–R2.7 run).
"""

from __future__ import annotations

import json
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

from .corpus import DEFAULT_MICRO_SUITE
from .scorer import MetricAtK, dedup_first, macro_average, score_query

#: Per-column ``bm25()`` weight order — the indexed FTS5 columns of ``schema::CREATE_SYMBOLS``.
COLUMN_ORDER: tuple[str, ...] = (
    "symbol_name",
    "symbol_type",
    "chunk_text",
    "parent_symbol",
    "imports",
    "cross_references",
    "file_docstring",
)


@dataclass(frozen=True)
class WeightVector:
    """One labelled point in the sweep grid: a 7-tuple of per-column ``bm25()`` weights."""

    label: str
    weights: tuple[float, ...]


#: The R2.2b grid (D23 weight-ablation axis); ``default`` is the shipped baseline (R2.2a).
DEFAULT_GRID: list[WeightVector] = [
    WeightVector("default", (10, 1, 1, 5, 2, 2, 2)),
    WeightVector("flat", (1, 1, 1, 1, 1, 1, 1)),
    WeightVector("name_only", (10, 0, 0, 0, 0, 0, 0)),
    WeightVector("body_heavy", (1, 1, 10, 1, 1, 1, 1)),
    WeightVector("name_strong", (20, 1, 1, 5, 2, 2, 2)),
    WeightVector("enrich_heavy", (10, 1, 1, 5, 5, 5, 5)),
]


@dataclass(frozen=True)
class SweepQuery:
    """One gold-labelled query from the shared micro-suite."""

    corpus_id: str
    query_id: str
    query: str
    query_type: str
    gold_files: frozenset[str]
    gold_blocks: frozenset[tuple[str, str]]


def load_suite(micro_suite_path: Path = DEFAULT_MICRO_SUITE) -> list[SweepQuery]:
    """Parse every corpus's queries + gold from the shared micro-suite fixture."""
    data = json.loads(Path(micro_suite_path).read_text(encoding="utf-8"))
    out: list[SweepQuery] = []
    for corpus in data["corpora"]:
        for q in corpus["queries"]:
            out.append(
                SweepQuery(
                    corpus_id=corpus["id"],
                    query_id=q["id"],
                    query=q["query"],
                    query_type=q.get("query_type", "keyword"),
                    gold_files=frozenset(q.get("gold_files", [])),
                    gold_blocks=frozenset((b["file_path"], b["symbol_name"]) for b in q.get("gold_blocks", [])),
                )
            )
    return out


#: A retrieval callable: ``(query, weight-tuple) -> object`` exposing best-first ``files``/``blocks``.
QueryFn = Callable[["SweepQuery", "tuple[float, ...]"], object]


@dataclass(frozen=True)
class VectorResult:
    """One ablation row: a weight vector's macro-averaged Layer-1 metrics (all + keyword-only)."""

    label: str
    weights: tuple[float, ...]
    macro_all: dict[int, MetricAtK]
    macro_keyword: dict[int, MetricAtK]
    n_queries: int
    n_keyword: int


def score_vectors(
    queries: Sequence[SweepQuery],
    grid: Sequence[WeightVector],
    query_fn: QueryFn,
) -> list[VectorResult]:
    """Score each grid vector over all queries; macro-average (all + keyword-only).

    ``query_fn(sweep_query, weights)`` returns an object exposing best-first ``files`` (str) and
    ``blocks`` (``(file_path, symbol_name)``) lists — the same shape the codecache adapter yields.
    File lists are de-duplicated first-seen (mirroring the Rust ``score_corpus`` fold) before scoring.
    """
    results: list[VectorResult] = []
    for vec in grid:
        per_query_all: list[list[MetricAtK]] = []
        per_query_kw: list[list[MetricAtK]] = []
        for sq in queries:
            retrieved = query_fn(sq, vec.weights)
            metrics = score_query(
                dedup_first(retrieved.files),
                list(retrieved.blocks),
                set(sq.gold_files),
                set(sq.gold_blocks),
            )
            per_query_all.append(metrics)
            if sq.query_type == "keyword":
                per_query_kw.append(metrics)
        results.append(
            VectorResult(
                label=vec.label,
                weights=tuple(vec.weights),
                macro_all=macro_average(per_query_all),
                macro_keyword=macro_average(per_query_kw),
                n_queries=len(per_query_all),
                n_keyword=len(per_query_kw),
            )
        )
    return results


def rank_vectors(
    results: Sequence[VectorResult],
    *,
    k: int = 10,
    key: str = "ndcg_block",
    keyword_only: bool = False,
) -> list[VectorResult]:
    """Rank vectors best-first by a macro metric at ``k`` (stable: ties keep grid order)."""

    def metric(vr: VectorResult) -> float:
        macro = vr.macro_keyword if keyword_only else vr.macro_all
        return getattr(macro[k], key)

    return sorted(results, key=metric, reverse=True)
