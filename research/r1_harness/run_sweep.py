"""Run the R2.2b BM25 weight sweep against the real codecache binary (zero-cost, local).

Materialises each micro-suite corpus once, indexes it, then for every weight vector in
``sweep.DEFAULT_GRID`` runs all 15 gold-labelled queries with ``codecache query --bm25-weights``,
scores Layer-1 (Recall/Precision/F1 + NDCG@10) and macro-averages into one ablation row per vector.
Prints a minimal table (R2.4 owns the polished reporter) and writes ``runs/sweep/report.json``.

Pure research/, no crate change, no paid spend. Directional signal on a 15-query PROXY corpus —
NOT a published weights finding (that is the gated R2.5–R2.7 external-corpus run).

Run (from research/r1_harness/, release binary built — ``cargo build --release``):
    PYTHONUTF8=1 C:/ccr1/Scripts/python.exe run_sweep.py
"""

from __future__ import annotations

import json
import sys
from dataclasses import asdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

HERE = Path(__file__).resolve().parent


def _fmt_w(w: float) -> str:
    f = float(w)
    return str(int(f)) if f.is_integer() else str(f)


def _print_table(results, rank_vectors) -> None:
    hdr = f"{'vector':13} {'weights':24} {'R@1':>5} {'R@10':>5} {'F1@10':>6} {'NDCG@10':>8} {'NDCG@10·kw':>11}"
    print("ranked by NDCG@10 (block granularity); all 15 queries unless noted ·kw = 13 keyword only:")
    print(hdr)
    for r in rank_vectors(results, k=10, key="ndcg_block"):
        b1, b10, kw10 = r.macro_all[1], r.macro_all[10], r.macro_keyword[10]
        wv = ",".join(_fmt_w(w) for w in r.weights)
        flag = "  <- baseline (shipped default)" if r.label == "default" else ""
        print(
            f"{r.label:13} {wv:24} {b1.recall_block:5.2f} {b10.recall_block:5.2f} "
            f"{b10.f1_block:6.2f} {b10.ndcg_block:8.3f} {kw10.ndcg_block:11.3f}{flag}"
        )


def main() -> int:
    from r1harness.codecache_tool import CodeCacheIndex, find_codecache_binary
    from r1harness.corpus import load_corpus, materialize
    from r1harness.sweep import COLUMN_ORDER, DEFAULT_GRID, load_suite, rank_vectors, score_vectors

    binary = find_codecache_binary()
    queries = load_suite()
    n_keyword = sum(1 for q in queries if q.query_type == "keyword")
    runs_dir = HERE / "runs" / "sweep"
    runs_dir.mkdir(parents=True, exist_ok=True)

    corpus_ids = sorted({q.corpus_id for q in queries})
    print(f"=== R2.2b BM25 weight sweep — binary={binary.name} ===")
    print(
        f"{len(queries)} queries ({n_keyword} keyword) across {len(corpus_ids)} corpora; {len(DEFAULT_GRID)} weight vectors"
    )
    print("indexing each corpus once (weights only affect query-time ranking)...\n")

    # Materialise + index each corpus ONCE; reuse across every weight vector.
    indices: dict[str, CodeCacheIndex] = {}
    for cid in corpus_ids:
        repo = runs_dir / cid / "repo"
        repo.mkdir(parents=True, exist_ok=True)
        materialize(load_corpus(cid), repo)
        idx = CodeCacheIndex(repo, binary)
        idx.init()
        idx.index()
        indices[cid] = idx

    def query_fn(sq, weights):
        return indices[sq.corpus_id].query(sq.query, bm25_weights=list(weights))

    results = score_vectors(queries, DEFAULT_GRID, query_fn)

    report = {
        "binary": str(binary),
        "n_queries": len(queries),
        "n_keyword": n_keyword,
        "column_order": list(COLUMN_ORDER),
        "grid": [{"label": v.label, "weights": list(v.weights)} for v in DEFAULT_GRID],
        "vectors": [
            {
                "label": r.label,
                "weights": list(r.weights),
                "n_queries": r.n_queries,
                "n_keyword": r.n_keyword,
                "macro_all": {str(k): asdict(m) for k, m in r.macro_all.items()},
                "macro_keyword": {str(k): asdict(m) for k, m in r.macro_keyword.items()},
            }
            for r in results
        ],
    }
    (runs_dir / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")

    _print_table(results, rank_vectors)
    print(f"\nreport: {runs_dir / 'report.json'}")
    print(
        "(Directional signal on a 15-query PROXY micro-suite — NOT a published finding; R2.5–R2.7 gate the real corpus.)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
