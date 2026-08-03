#!/usr/bin/env python3
"""Reject identity changes and mutations of already-published marketplace versions."""

from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path


def git(*args: str, text: bool = True) -> str | bytes:
    return subprocess.check_output(["git", *args], text=text)


def from_git(base: str, path: str) -> dict:
    raw = git("show", f"{base}:{path}", text=False)
    return tomllib.loads(raw.decode("utf-8"))


def from_worktree(path: str) -> dict:
    return tomllib.loads(Path(path).read_text(encoding="utf-8"))


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_marketplace_diff.py BASE_SHA", file=sys.stderr)
        return 2
    base = sys.argv[1]
    diff = git(
        "diff",
        "--name-status",
        "--find-renames",
        base,
        "--",
        "marketplace/plugins",
    )
    failures: list[str] = []
    for line in diff.splitlines():
        if not line.strip():
            continue
        parts = line.split("\t")
        status = parts[0]
        paths = parts[1:]
        path = paths[-1]
        if status.startswith(("D", "R", "T")):
            failures.append(
                f"published marketplace paths cannot be deleted, renamed, or change file type: {path}"
            )
            continue
        if not status.startswith("M"):
            continue
        if path.endswith("/plugin.toml"):
            before = from_git(base, path)
            after = from_worktree(path)
            for field in ("id", "type", "repository_id"):
                if before.get(field) != after.get(field):
                    failures.append(f"{path}: immutable field '{field}' changed")
            continue
        if "/versions/" not in path or not path.endswith(".toml"):
            continue
        before = from_git(base, path)
        after = from_worktree(path)
        before_yanked = bool(before.pop("yanked", False))
        after_yanked = bool(after.pop("yanked", False))
        if before != after:
            failures.append(
                f"{path}: published version metadata is immutable; add a new SemVer instead"
            )
        if before_yanked and not after_yanked:
            failures.append(f"{path}: a yanked version cannot be restored in place")

    if failures:
        for failure in failures:
            print(f"error: {failure}", file=sys.stderr)
        return 1
    print("marketplace identity and published-version immutability checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
