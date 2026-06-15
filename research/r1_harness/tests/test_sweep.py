"""Unit tests for the R2.2b BM25 weight-sweep apparatus (pure; no binary).

The sweep's I/O (materialise -> index -> query the real binary) is exercised end-to-end by
run_sweep.py; here we pin the pure logic: the grid shape, parsing the shared micro_suite.json
into gold-labelled queries, and the per-vector macro aggregation + ranking that turns canned
retrievals into one ablation row per weight vector.
"""

from r1harness.sweep import (
    DEFAULT_GRID,
    SweepQuery,
    WeightVector,
    load_suite,
    rank_vectors,
    score_vectors,
)


def test_default_grid_is_wellformed():
    assert len(DEFAULT_GRID) == 6
    labels = [v.label for v in DEFAULT_GRID]
    assert len(set(labels)) == len(labels)  # labels are unique
    for v in DEFAULT_GRID:
        assert len(v.weights) == 7  # one per indexed FTS5 column
    default = next(v for v in DEFAULT_GRID if v.label == "default")
    assert tuple(default.weights) == (10, 1, 1, 5, 2, 2, 2)  # the shipped baseline (R2.2a)


def test_load_suite_parses_shared_micro_suite():
    queries = load_suite()
    # 3 corpora x 5 queries = 15 gold-labelled queries
    assert len(queries) == 15
    by_id = {q.query_id: q for q in queries}
    q1 = by_id["auth_q1"]
    assert q1.corpus_id == "auth_module"
    assert q1.query_type == "keyword"
    assert set(q1.gold_files) == {"src/auth/authenticate.py"}
    assert set(q1.gold_blocks) == {("src/auth/authenticate.py", "authenticate_user")}
    # the two deliberately-semantic BM25-gap queries (auth_q5, config_q5) vs 13 keyword
    assert sum(1 for q in queries if q.query_type == "keyword") == 13
    assert sum(1 for q in queries if q.query_type == "semantic") == 2


# --- score_vectors aggregation + ranking, with a canned (no-binary) query_fn ---

PERFECT = (1, 0, 0, 0, 0, 0, 0)
MISS = (0, 0, 0, 0, 0, 0, 1)


class _Retrieved:
    """Minimal duck-typed stand-in for a QueryResult (best-first files/blocks)."""

    def __init__(self, files, blocks):
        self.files = files
        self.blocks = blocks


def _tiny_suite():
    return [
        SweepQuery("c", "k1", "q one", "keyword", frozenset({"a.py"}), frozenset({("a.py", "f")})),
        SweepQuery("c", "k2", "q two", "keyword", frozenset({"b.py"}), frozenset({("b.py", "g")})),
        SweepQuery("c", "s1", "q three", "semantic", frozenset({"d.py"}), frozenset({("d.py", "h")})),
    ]


def _canned_query_fn(sq, weights):
    # the PERFECT vector surfaces exactly the gold; every other vector surfaces nothing.
    if tuple(weights) == PERFECT:
        return _Retrieved(sorted(sq.gold_files), sorted(sq.gold_blocks))
    return _Retrieved([], [])


def test_score_vectors_macro_and_keyword_split():
    grid = [WeightVector("perfect", PERFECT), WeightVector("miss", MISS)]
    results = score_vectors(_tiny_suite(), grid, _canned_query_fn)
    assert [r.label for r in results] == ["perfect", "miss"]  # grid order preserved
    perfect, miss = results
    assert perfect.n_queries == 3
    assert perfect.n_keyword == 2  # 2 keyword + 1 semantic
    # perfect vector ranks the single gold block first => NDCG@10 / recall@10 == 1.0
    assert perfect.macro_all[10].ndcg_block == 1.0
    assert perfect.macro_all[10].recall_block == 1.0
    # miss vector surfaces nothing => 0.0 (gold is non-empty for all three queries)
    assert miss.macro_all[10].recall_block == 0.0
    assert miss.macro_all[10].ndcg_block == 0.0


def test_rank_vectors_orders_by_ndcg_block():
    # input order deliberately worst-first; ranking must reorder best-first by NDCG@10 block.
    grid = [WeightVector("miss", MISS), WeightVector("perfect", PERFECT)]
    results = score_vectors(_tiny_suite(), grid, _canned_query_fn)
    ranked = rank_vectors(results, k=10, key="ndcg_block")
    assert [r.label for r in ranked] == ["perfect", "miss"]
