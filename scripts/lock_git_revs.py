#!/usr/bin/env python3
"""Fail when Cargo.lock links one alice-* package from more than one git revision.

Siblings that pin the same git dependency at different revisions make cargo link
that crate once per revision: the build passes, but the same crate is compiled
and linked several times, and its types are not interchangeable between the
copies. Run it after `cargo test`, so the lock checked is the one the build used.

  lock_git_revs.py [Cargo.lock]

Exit 0 when every alice-* git package comes from a single revision, 1 when one
comes from several (each is named with its revisions), 2 when the lock has no
alice-* git package at all (nothing was compared) or cannot be read.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


def git_rev(source: str) -> str | None:
    """The resolved commit of a `git+...#<commit>` source, else None."""
    if not source.startswith("git+") or "#" not in source:
        return None
    return source.rsplit("#", 1)[1]


def revisions(lock: dict) -> dict[str, set[str]]:
    """alice-* package name -> the git commits it is linked from."""
    found: dict[str, set[str]] = {}
    for pkg in lock.get("package", []):
        name = pkg.get("name", "")
        rev = git_rev(pkg.get("source", ""))
        if name.startswith("alice-") and rev:
            found.setdefault(name, set()).add(rev)
    return found


def check(lock: dict) -> tuple[int, list[str]]:
    found = revisions(lock)
    errors = [
        f"{name} is linked from {len(revs)} git revisions: {', '.join(sorted(r[:12] for r in revs))}"
        for name, revs in sorted(found.items())
        if len(revs) > 1
    ]
    return len(found), errors


def main(argv: list[str]) -> int:
    path = Path(argv[1] if len(argv) > 1 else "Cargo.lock")
    try:
        lock = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as e:
        print(f"error: cannot read {path}: {e}")
        return 2
    compared, errors = check(lock)
    print(f"compared: alice-* git packages {compared}")
    if compared == 0:
        print(f"error: {path} has no alice-* git package; nothing was compared")
        return 2
    for e in errors:
        print(f"error: {e}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
