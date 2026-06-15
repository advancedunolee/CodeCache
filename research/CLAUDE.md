# research/ — CLAUDE.md

Research track (R1–R4) artifacts. **Owner:** `research-harness-engineer` agent for **R2+** (ROADMAP
**D23**, adopted 2026-06-14 — sonnet; scope `research/`; gates **ruff + pytest**; process-boundary to the
binary). The main session drove **R1** (D22). The `principal-engineering-manager` stays gatekeeper
(scope/DoD/doc-sync) and the `code-reviewer` is the independent APPROVE/BLOCK gate.

## What lives here
- `r1_harness/` — the shared research harness (Python). Holds **both** tracks (one package, `r1harness/`):
  - **R1 eval harness** — a fork of mini-SWE-agent that runs the same-agent retrieval-interface ablation
    (arms A0/A1/A4) against the built `codecache` binary and scores Layer-1/Layer-2 metrics from trajectory
    logs. See `r1_harness/README.md`.
  - **R2 offline ablation apparatus** (D23) — pure, binary-via-process-boundary modules that reuse the same
    `corpus.py`/`scorer.py`/`codecache_tool.py`: `scorer.py` (NDCG@10, R2.1), `sweep.py`+`run_sweep.py` (BM25
    weight sweep, R2.2b), `chunkers.py`+`ab_runner.py`+`run_ab.py` (R2.3b stub chunker + native-vs-stub
    A/B plumbing over the D25 `codecache ingest` seam — holds storage/FTS5/retriever/enrichment constant so
    the chunker is the only ablated axis; astchunk/cAST drops into the same plumbing at the gated R2.6),
    `ablation_report.py`+`run_report.py` (R2.4 ablation-table reporter — aggregates sweep + A/B into a single
    Markdown view with n_queries-weighted A/B aggregation, directional top-config selection, and scope-honesty
    disclaimer; pure core + thin loaders + entrypoint), and `contextbench.py`+`fetch_contextbench.py`
    (R2.5a external-corpus loader — pure mapper core + thin fetch entrypoint for the ContextBench-Lite HF
    dataset; see "External-corpus provenance" below).
    Real-run outputs land under `r1_harness/runs/`.

## Rules (different from the Rust crate)
- **Out-of-crate, research-only.** Nothing here is a Rust dependency, ships in a release artifact,
  or touches `Cargo.toml`. The four Rust gates (fmt/clippy/test/build) do not apply; this is Python.
- **Process boundary only.** The harness talks to CodeCache by shelling out to the `codecache`
  binary — no FFI/PyO3. Preserves the zero-dependency single-binary identity (D12/D15).
- **One gold source.** Layer-1 gold contexts come from `tests/fixtures/retrieval_quality/`
  (shared with the Rust M10.2 scorer); the Python scorer ports the M10.2 protocol verbatim (D21).
- **No paid spend without a gate.** R1 runs offline (deterministic/local model). The ~$1K R3 API
  spend and any paid benchmark/API access are separate downstream human gates.
  **EXCEPTION (D26 ratified):** the `fetch_contextbench.py` entrypoint (R2.5a) makes a **one-time,
  cached, no-auth-token** download from HF (`Contextbench/ContextBench`) — zero paid spend, authorized
  for the research harness only. The **product (codecache binary) stays fully air-gapped**.
  The test suite remains hermetic — it never triggers a network call.
- **Scope discipline (`../project_overview.md` §7):** R1 builds outcome-agnostic apparatus; arm
  winners are an R3 determination, not R1.

## Python environment decision (R2.5a, recorded 2026-06-15)
System Python (`/usr/bin/python3` 3.12.3) is externally managed (PEP 668 / Debian policy).
**Decision: use a project-local venv** at `research/r1_harness/.venv/` (gitignored).
Rationale: avoids `--break-system-packages`; keeps deps isolated; mirrors the Windows `C:\ccr1` venv pattern.
Create with:
```
python3 -m venv research/r1_harness/.venv
research/r1_harness/.venv/bin/pip install -r research/r1_harness/requirements.txt
```
Gate commands use the venv Python:
```
PYTHONUTF8=1 research/r1_harness/.venv/bin/pytest research/r1_harness/
research/r1_harness/.venv/bin/ruff check research/
research/r1_harness/.venv/bin/ruff format --check research/
```
Note: `datasets` and `huggingface_hub` are pinned in `requirements.txt` but are **NOT installed**
in the venv as of R2.5a-rev (the venv contains only pytest + ruff). They are required only for
the fetch entrypoint (`fetch_contextbench.py`); the core mapper and test suite are hermetic and
do NOT import them. Install only when ready to run the fetch entrypoint:
```
research/r1_harness/.venv/bin/pip install datasets==5.0.0 huggingface_hub==1.19.0
```
The test suite remains hermetic (green) whether or not these deps are installed.

## External-corpus provenance (R2.5a, D26)
**ContextBench-Lite** (`r1harness/contextbench.py`, `fetch_contextbench.py`):
- Source: HF dataset `Contextbench/ContextBench`, config `contextbench_verified` (500-task subset).
- License: **Apache-2.0** (confirmed: github.com/EuniAI/ContextBench). arXiv:2602.05892.
- Download: one-time cached to `r1_harness/cache/contextbench/` (gitignored — do NOT commit blobs).
- No auth token required. No paid spend.
- Attribution: EuniAI / ContextBench team.

**CodeRAG-Bench RepoEval** (R2.5b, NOT YET BUILT):
- Source: github.com/code-rag-bench/code-rag-bench, arXiv:2406.14497 (NAACL'25).
- License: Per paper/release, expected CC-BY-SA 4.0. **LICENSE file status as of 2026-06-15:
  the GitHub repo root contains no LICENSE file** (GitHub API `/license` endpoint returns 404;
  root listing shows only README.md, generation/, preprocessor/, requirements.txt, retrieval/).
  The CC-BY-SA 4.0 claim comes from the paper/release notes — **not confirmed via LICENSE file**.
  **FIRST STEP of R2.5b: re-check for a LICENSE file (repo may have added one) and record it.**
  Do NOT load any CodeRAG-Bench data until license is confirmed.
- RepoEval/RepoCoder underlying data: MIT.

## Update rule
Code change here ⇒ update `docs/TODO.md` (research-track section) in the same change, mirroring the
crate's golden rule.
