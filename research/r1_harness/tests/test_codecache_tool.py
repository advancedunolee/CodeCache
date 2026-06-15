"""Unit tests for the codecache tool adapter's pure parsing/normalisation.

These need no binary — they exercise §6.4.2 JSON parsing and the path
relativisation that makes retrieved paths gold-comparable. The end-to-end run
against the real binary is verified separately (see README "Running").
"""

import json
import sys

import pytest

from r1harness.codecache_tool import build_query_args, normalize_path, parse_query_json


def test_normalize_absolute_path_under_repo(tmp_path):
    abs_fp = tmp_path / "src" / "auth" / "authenticate.py"
    abs_fp.parent.mkdir(parents=True)
    abs_fp.write_text("x", encoding="utf-8")
    assert normalize_path(str(abs_fp), tmp_path) == "src/auth/authenticate.py"


@pytest.mark.skipif(
    sys.platform != "win32", reason="Windows-specific path semantics: backslash is a path separator only on Windows"
)
def test_normalize_relative_path_backslashes_to_posix():
    assert normalize_path("src\\auth\\authenticate.py", None) == "src/auth/authenticate.py"


def test_normalize_path_not_under_repo_falls_back(tmp_path):
    # an unrelated absolute path is posix-normalised, not crashed on
    out = normalize_path("other/place/file.py", tmp_path)
    assert out == "other/place/file.py"


def test_parse_query_json_dedups_files_keeps_block_order(tmp_path):
    payload = {
        "query": "authenticate user credentials",
        "total_results": 3,
        "total_tokens": 280,
        "chunks": [
            {
                "symbol_name": "authenticate_user",
                "file_path": str(tmp_path / "src/auth/authenticate.py"),
                "symbol_type": "function",
                "language": "python",
                "bm25_score": 9.1,
                "chunk_text": "def authenticate_user(): ...",
            },
            {
                "symbol_name": "verify_password",
                "file_path": str(tmp_path / "src/auth/authenticate.py"),
                "symbol_type": "function",
                "language": "python",
                "bm25_score": 4.0,
                "chunk_text": "def verify_password(): ...",
            },
            {
                "symbol_name": "generate_session_token",
                "file_path": str(tmp_path / "src/auth/session.py"),
                "symbol_type": "function",
                "language": "python",
                "bm25_score": 2.0,
                "chunk_text": "def generate_session_token(): ...",
            },
        ],
    }
    qr = parse_query_json(json.dumps(payload), "authenticate user credentials", repo_dir=tmp_path)
    # file list dedups authenticate.py to one entry, first-seen order
    assert qr.files == ["src/auth/authenticate.py", "src/auth/session.py"]
    # block list keeps all three, best-first order, relativised
    assert qr.blocks[0] == ("src/auth/authenticate.py", "authenticate_user")
    assert len(qr.blocks) == 3
    assert qr.total_tokens == 280


# --- R2.2b: --bm25-weights threading through the query CLI (per-column BM25 sweep) ---

# Shipped default per-column weights (schema::CREATE_SYMBOLS indexed-column order):
# symbol_name, symbol_type, chunk_text, parent_symbol, imports, cross_references, file_docstring.
DEFAULT_BM25_WEIGHTS = [10, 1, 1, 5, 2, 2, 2]


def test_build_query_args_default_omits_weights_flag():
    args = build_query_args("authenticate user", max_tokens=4000, max_results=20)
    assert args[0] == "query"
    assert args[1] == "authenticate user"
    assert "--format" in args and "json" in args
    # absent weights => no flag => the binary uses its built-in defaults (byte-identical path)
    assert "--bm25-weights" not in args


def test_build_query_args_threads_weights_in_schema_order():
    args = build_query_args("authenticate user", max_tokens=4000, max_results=20, bm25_weights=DEFAULT_BM25_WEIGHTS)
    i = args.index("--bm25-weights")
    # one comma-joined value: 7 finite floats, schema column order, no spaces
    assert args[i + 1] == "10.0,1.0,1.0,5.0,2.0,2.0,2.0"


def test_build_query_args_rejects_wrong_arity():
    # the sweep generates vectors programmatically; a wrong-length vector must fail loudly
    # in Python, not as an opaque subprocess error from the binary's own validation.
    with pytest.raises(ValueError):
        build_query_args("q", max_tokens=4000, max_results=20, bm25_weights=[10, 1, 1])
