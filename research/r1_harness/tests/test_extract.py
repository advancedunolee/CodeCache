"""Unit tests for the surfaced-items extractor (pure; no agent/binary)."""

from pathlib import Path

from r1harness.extract import extract_surfaced, is_codecache_query

REPO_FILES = {"src/auth/authenticate.py", "src/auth/session.py"}


def test_is_codecache_query():
    assert is_codecache_query('codecache query "x" --format json')
    assert not is_codecache_query("grep -rn authenticate src")
    assert not is_codecache_query("codecache index")


def test_codecache_json_is_authoritative(tmp_path):
    payload = (
        '{"query":"q","total_results":1,"total_tokens":42,"chunks":['
        '{"symbol_name":"authenticate_user","file_path":"' + (tmp_path / "src/auth/authenticate.py").as_posix() + '",'
        '"symbol_type":"function","language":"python","bm25_score":9.0,"chunk_text":"def authenticate_user(): ..."}]}'
    )
    files, blocks = extract_surfaced('codecache query "authenticate user" --format json', payload, REPO_FILES, tmp_path)
    assert files == ["src/auth/authenticate.py"]
    assert blocks == [("src/auth/authenticate.py", "authenticate_user")]


def test_grep_def_lines_give_precise_blocks():
    obs = (
        "src/auth/authenticate.py:1:def authenticate_user(username, password):\n"
        "src/auth/authenticate.py:11:def verify_password(plaintext, hashed):\n"
        "src/auth/session.py:1:def generate_session_token(user_id):\n"
    )
    files, blocks = extract_surfaced("grep -rn def src", obs, REPO_FILES)
    assert "src/auth/authenticate.py" in files
    assert ("src/auth/authenticate.py", "authenticate_user") in blocks
    assert ("src/auth/session.py", "generate_session_token") in blocks


def test_cat_attributes_shown_defs_to_the_file():
    obs = "def authenticate_user(username, password):\n    ...\ndef verify_password(p, h):\n    ...\n"
    files, blocks = extract_surfaced("cat src/auth/authenticate.py", obs, REPO_FILES)
    assert files == ["src/auth/authenticate.py"]
    assert ("src/auth/authenticate.py", "authenticate_user") in blocks
    assert ("src/auth/authenticate.py", "verify_password") in blocks


def test_unrelated_command_surfaces_nothing():
    files, blocks = extract_surfaced("ls -la", "total 0\n", REPO_FILES)
    assert files == []
    assert blocks == []
