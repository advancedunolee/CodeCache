"""A bash-backed environment for mini-SWE-agent (portable agent shell).

mini-SWE-agent's stock ``LocalEnvironment`` runs ``action["command"]`` with
``shell=True`` — which on **Windows is cmd.exe**, not bash. The retrieval arms
(A0 grep/glob/read; A1 ``codecache query``) assume bash semantics (globbing,
quoting, pipes), so this subclass runs commands through ``bash -c`` directly
(``shell=False`` — no cmd double-quoting layer), keeping the same return-dict
contract and the ``COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT`` submit detection.

On Linux/macOS this is equivalent to the stock environment; on Windows it uses
the Git-for-Windows ``bash.exe`` (MSYS grep/cat/etc. are already on PATH).
"""

from __future__ import annotations

import os
import shutil
import subprocess
from typing import Any

from minisweagent.environments.local import LocalEnvironment


class BashEnvironment(LocalEnvironment):
    """LocalEnvironment that executes via ``bash -c`` regardless of host OS."""

    def __init__(self, *, bash_path: str | None = None, extra_path: str | None = None, **kwargs) -> None:
        super().__init__(**kwargs)
        self.bash = bash_path or shutil.which("bash")
        if not self.bash:
            raise FileNotFoundError("bash not found on PATH; install Git for Windows or set bash_path.")
        # Prepend extra_path (e.g. the codecache binary dir) so `codecache ...` resolves like a user's agent.
        self.extra_path = extra_path

    def _child_env(self) -> dict[str, str]:
        env = os.environ | self.config.env
        if self.extra_path:
            env["PATH"] = self.extra_path + os.pathsep + env.get("PATH", "")
        env.setdefault("PYTHONUTF8", "1")
        return env

    def execute(self, action: dict, cwd: str = "", *, timeout: int | None = None) -> dict[str, Any]:
        command = action.get("command", "")
        cwd = cwd or self.config.cwd or os.getcwd()
        try:
            proc = subprocess.run(
                [self.bash, "-c", command],
                cwd=cwd,
                env=self._child_env(),
                text=True,
                encoding="utf-8",
                errors="replace",
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=timeout or self.config.timeout,
                check=False,
            )
            output = {"output": proc.stdout, "returncode": proc.returncode, "exception_info": ""}
        except subprocess.TimeoutExpired as e:
            raw = e.output or ""
            raw = raw.decode("utf-8", errors="replace") if isinstance(raw, bytes) else raw
            output = {
                "output": raw,
                "returncode": -1,
                "exception_info": f"command timed out after {timeout or self.config.timeout}s",
                "extra": {"exception_type": "TimeoutExpired", "exception": str(e)},
            }
        self._check_finished(output)  # raises Submitted on the submit sentinel (inherited)
        return output
