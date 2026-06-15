"""R2.5a — ContextBench-Lite fetch entrypoint.

Downloads the ``contextbench_verified`` (Lite, 500-task) split from the HF dataset
``Contextbench/ContextBench`` ONCE, writes a pinned cached slice as JSON under the
cache dir, and exits.  Subsequent runs skip re-download if the cache is already present.

This is the ONLY network surface in R2.5a.  The test suite (pytest) NEVER calls this
script — tests run against fixture data.  The pure-logic mapper (``r1harness/contextbench.py``)
has no I/O and is tested independently.

Usage:
    python3 fetch_contextbench.py [--cache-dir PATH] [--n-records N]

Options:
    --cache-dir PATH   Directory to write the cached slice (default: ./cache/contextbench)
    --n-records N      Number of records to cache (default: 20; full Lite = 500)
    --force            Re-download even if cache exists

Environment variables:
    CONTEXTBENCH_CACHE  Override default cache dir (same as --cache-dir)

Exit codes:
    0  Success (downloaded or cache already present)
    1  Error (network failure, missing deps, etc.)

Missing-cache behaviour for downstream scripts:
    If the cache does not exist, downstream scripts should call this entrypoint first.
    See the stderr message in load_cached_contextbench() for instructions.

Dataset:  HF ``Contextbench/ContextBench``, config ``contextbench_verified``.
License:  Apache-2.0.  arXiv:2602.05892.  No auth token required.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

DEFAULT_CACHE_DIR = Path(__file__).resolve().parent / "cache" / "contextbench"
DEFAULT_N_RECORDS = 20
CACHE_FILE_NAME = "contextbench_verified_slice.json"


def _cache_path(cache_dir: Path) -> Path:
    return cache_dir / CACHE_FILE_NAME


def load_cached_contextbench(cache_dir: Path | None = None) -> list[dict]:
    """Load the cached ContextBench-Lite slice as a list of dicts.

    Called by downstream scripts (not by the test suite).  If the cache is missing,
    prints a clear instruction to stderr and raises SystemExit(1).
    """
    resolved = Path(cache_dir) if cache_dir else DEFAULT_CACHE_DIR
    cp = _cache_path(resolved)
    if not cp.exists():
        print(
            "ERROR: ContextBench-Lite cache not found.\n"
            f"  Expected: {cp}\n"
            "  Run the fetch entrypoint first:\n"
            "    python3 fetch_contextbench.py\n"
            "  Then retry.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return json.loads(cp.read_text(encoding="utf-8"))


def fetch_and_cache(
    cache_dir: Path,
    n_records: int,
    force: bool = False,
) -> int:
    """Download the ContextBench-Lite slice and write it to ``cache_dir``.

    Returns 0 on success, 1 on failure.
    """
    cp = _cache_path(cache_dir)

    if cp.exists() and not force:
        print(f"Cache already present: {cp}", file=sys.stderr)
        return 0

    # Import the HF stack only here — keeps the mapper and test suite deps-free.
    try:
        from datasets import load_dataset  # type: ignore[import]
    except ImportError:
        print(
            "ERROR: 'datasets' package not installed.\n"
            "  Install it with:  pip install datasets==5.0.0 huggingface_hub==1.19.0\n"
            "  Then retry:       python3 fetch_contextbench.py",
            file=sys.stderr,
        )
        return 1

    print(
        f"Downloading ContextBench-Lite (contextbench_verified, up to {n_records} records) from HF (no auth token)...",
        file=sys.stderr,
    )
    try:
        ds = load_dataset(
            "Contextbench/ContextBench",
            name="contextbench_verified",
            split="train",
            trust_remote_code=False,
        )
    except Exception as exc:
        print(f"ERROR: download failed: {exc}", file=sys.stderr)
        return 1

    # Take a deterministic head slice.
    total = len(ds)
    take = min(n_records, total)
    slice_ds = ds.select(range(take))

    # Materialise to a list of plain dicts (JSON-serialisable).
    records: list[dict] = []
    for row in slice_ds:
        records.append(dict(row))

    cache_dir.mkdir(parents=True, exist_ok=True)
    cp.write_text(json.dumps(records, ensure_ascii=False, indent=2), encoding="utf-8")
    print(
        f"Cached {take}/{total} records to: {cp}",
        file=sys.stderr,
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Fetch and cache a ContextBench-Lite slice from HF.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=Path(os.environ.get("CONTEXTBENCH_CACHE", str(DEFAULT_CACHE_DIR))),
        help=f"Cache directory (default: {DEFAULT_CACHE_DIR}; env: CONTEXTBENCH_CACHE)",
    )
    parser.add_argument(
        "--n-records",
        type=int,
        default=DEFAULT_N_RECORDS,
        help=f"Number of records to cache (default: {DEFAULT_N_RECORDS}; full Lite = 500)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Re-download even if cache already exists",
    )
    args = parser.parse_args(argv)

    cache_dir: Path = args.cache_dir
    cp = _cache_path(cache_dir)

    if args.force:
        # Explicit download requested — fetch and cache (imports datasets only here).
        return fetch_and_cache(cache_dir=cache_dir, n_records=args.n_records, force=True)

    # Default read path (no --force): check the cache and instruct-and-exit if missing.
    # NEVER auto-download on the default path — mirror the run_report.py precedent.
    if not cp.exists():
        print(
            "ERROR: ContextBench-Lite cache not found.\n"
            f"  Expected: {cp}\n"
            "  Run the fetch entrypoint first:\n"
            "    python3 fetch_contextbench.py --force\n"
            "  Then retry.",
            file=sys.stderr,
        )
        return 1

    print(f"Cache already present: {cp}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
