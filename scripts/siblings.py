#!/usr/bin/env python3
"""Clone every sibling repository this crate reaches through path dependencies.

Cargo.toml names its dependencies as `path = "../ALICE-X"`, and those
manifests name further siblings the same way, so the set is the transitive
closure over every Cargo.toml reached (workspace members included). Each
repository is cloned once, shallow, at its default branch, next to this
checkout; the resolved commit of each is written to the summary file so a red
build names the sibling heads it was built against.

    python3 scripts/siblings.py [--parent DIR] [--owner NAME] [--jobs N] [--list]

--list prints the set without cloning. Fails when the set is empty (nothing
compared) or a clone fails.
"""
from __future__ import annotations

import argparse
import concurrent.futures as cf
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATH_DEP = re.compile(r'path\s*=\s*"([^"]+)"')


def manifest_paths(manifest: Path) -> list[Path]:
    """Directories named by `path = "…"` in one manifest (dependencies and members)."""
    text = manifest.read_text(encoding="utf-8")
    text = re.sub(r"(?m)#.*$", "", text)
    out = [(manifest.parent / m).resolve() for m in PATH_DEP.findall(text)]
    members = re.search(r"(?s)\bmembers\s*=\s*\[(.*?)\]", text)
    if members:
        out += [(manifest.parent / m).resolve() for m in re.findall(r'"([^"]+)"', members.group(1))]
    return out


def repo_of(path: Path, parent: Path) -> str | None:
    try:
        rel = path.relative_to(parent)
    except ValueError:
        return None
    return rel.parts[0] if rel.parts else None


def closure(start: Path, parent: Path, clone) -> list[str]:
    """Repository names reached from `start`; `clone(name)` makes one present."""
    start, parent = start.resolve(), parent.resolve()
    seen: set[str] = set()
    todo = [start / "Cargo.toml"]
    done: set[Path] = set()
    while todo:
        m = todo.pop()
        if m in done or not m.exists():
            continue
        done.add(m)
        for d in manifest_paths(m):
            name = repo_of(d, parent)
            if name is None or name == start.name:
                if (d / "Cargo.toml").exists():
                    todo.append(d / "Cargo.toml")
                continue
            if name not in seen:
                seen.add(name)
                clone(name)
            todo.append(d / "Cargo.toml")
    return sorted(seen)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--parent", type=Path, default=ROOT.parent)
    ap.add_argument("--owner", default="ext-sakamoro")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--summary", type=Path)
    a = ap.parse_args(argv)
    parent = a.parent.resolve()
    failed: list[str] = []
    pool = cf.ThreadPoolExecutor(a.jobs)
    pending: dict[str, cf.Future] = {}

    def clone(name: str) -> None:
        if a.list or (parent / name).exists():
            return
        url = f"https://github.com/{a.owner}/{name}.git"
        r = subprocess.run(["git", "clone", "-q", "--depth", "1", url, str(parent / name)],
                           capture_output=True, text=True)
        if r.returncode != 0:
            failed.append(f"{name}: {r.stderr.strip()}")

    # clone synchronously per name so its manifest can be read next (the
    # breadth is in the first level, which is cloned in parallel up front)
    first = {repo_of(d, parent) for d in manifest_paths(ROOT / "Cargo.toml")} - {None, ROOT.name}
    if not a.list:
        list(pool.map(clone, sorted(first)))
    names = closure(ROOT, parent, clone)
    pool.shutdown()
    if a.summary:
        lines = []
        for n in names:
            head = subprocess.run(["git", "-C", str(parent / n), "rev-parse", "--short=12", "HEAD"],
                                  capture_output=True, text=True).stdout.strip()
            lines.append(f"{n} {head}")
        a.summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    for f in failed:
        print(f"error: clone failed: {f}", file=sys.stderr)
    print(f"siblings: {len(names)} repositories")
    if not names:
        print("error: no sibling found (compared nothing)", file=sys.stderr)
        return 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
