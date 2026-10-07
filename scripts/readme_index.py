#!/usr/bin/env python3
"""Keep the crate index in README.md consistent with docs/crate-index.tsv and
with the repositories it points to.

  --write    render the index tables from docs/crate-index.tsv into README.md
             (between the crate-index markers)
  --check    offline: README.md contains exactly the rendered tables, the TSV
             is well formed (no duplicates, known columns), and every relative
             link in README.md / docs/DEMOS.md resolves to a tracked file
  --online   compare docs/crate-index.tsv with the live sources:
               * each repository exists, is public and not archived
               * it is a Rust crate (Cargo.toml at the root) and not a hosted
                 service template (docker-compose.yml + database/ + frontend/)
               * the license column equals the Cargo.toml license
                 ([package], [workspace.package], or the member crate)
               * the crates.io column matches whether the crate is published
               * no public ALICE-* crate repository is missing from the index
             needs network; uses GITHUB_TOKEN / GH_TOKEN when set

Every mode fails when it compared nothing, so a renamed file or marker cannot
turn the check into a no-op.
"""

from __future__ import annotations

import json
import os
import re
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TSV = Path("docs/crate-index.tsv")
README = Path("README.md")
LINKED_DOCS = (Path("README.md"), Path("docs/DEMOS.md"))
START = "<!-- crate-index:start -->"
END = "<!-- crate-index:end -->"
OWNER = "ext-sakamoro"
SELF = "ALICE-Eco-System"
COLUMNS = ("category", "repository", "crate", "crates.io", "license", "description")


def read(root: Path, rel: Path) -> str:
    return (root / rel).read_text(encoding="utf-8")


def load_rows(root: Path) -> list[dict[str, str]]:
    rows = []
    for no, line in enumerate(read(root, TSV).splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        cells = line.split("\t")
        if len(cells) != len(COLUMNS):
            raise SystemExit(f"error: {TSV}:{no}: expected {len(COLUMNS)} columns, got {len(cells)}")
        row = dict(zip(COLUMNS, cells))
        if row["crates.io"] not in ("", "yes"):
            raise SystemExit(f"error: {TSV}:{no}: crates.io column must be empty or 'yes'")
        if row["crates.io"] == "yes" and not row["crate"]:
            raise SystemExit(f"error: {TSV}:{no}: published row needs a crate name")
        if not row["repository"].startswith("ALICE-"):
            raise SystemExit(f"error: {TSV}:{no}: repository must be an ALICE-* name")
        for key in ("category", "license", "description"):
            if not row[key]:
                raise SystemExit(f"error: {TSV}:{no}: empty {key}")
        rows.append(row)
    return rows


def render(rows: list[dict[str, str]]) -> str:
    out = [START, ""]
    category = None
    for row in rows:
        if row["category"] != category:
            if category is not None:
                out.append("")
            category = row["category"]
            out += [f"### {category}", "", "| Repository | Crate | License | Description |", "|---|---|---|---|"]
        repo = f"[{row['repository']}](https://github.com/{OWNER}/{row['repository']})"
        if row["crates.io"]:
            crate = (f"[![crates.io](https://img.shields.io/crates/v/{row['crate']}.svg)]"
                     f"(https://crates.io/crates/{row['crate']})")
        elif row["crate"]:
            crate = f"`{row['crate']}`"
        else:
            crate = "workspace"
        out.append(f"| {repo} | {crate} | {row['license']} | {row['description']} |")
    out += ["", END]
    return "\n".join(out)


def split_readme(text: str) -> tuple[str, str, str]:
    if text.count(START) != 1 or text.count(END) != 1:
        raise SystemExit(f"error: {README} must contain each crate-index marker exactly once")
    head, rest = text.split(START, 1)
    body, tail = rest.split(END, 1)
    return head, START + body + END, tail


def check_offline(root: Path) -> list[str]:
    errors = []
    rows = load_rows(root)
    seen: dict[str, int] = {}
    for row in rows:
        seen[row["repository"]] = seen.get(row["repository"], 0) + 1
    errors += [f"duplicate repository in {TSV}: {r}" for r, n in seen.items() if n > 1]
    # categories must be contiguous so each renders as one table
    order = [r["category"] for r in rows]
    for cat in set(order):
        idx = [i for i, c in enumerate(order) if c == cat]
        if idx != list(range(idx[0], idx[-1] + 1)):
            errors.append(f"category '{cat}' is split in {TSV}; keep its rows together")
    _, block, _ = split_readme(read(root, README))
    if block != render(rows):
        errors.append(f"{README} crate index differs from {TSV}; run scripts/readme_index.py --write")
    links = 0
    for doc in LINKED_DOCS:
        if not (root / doc).exists():
            errors.append(f"missing {doc}")
            continue
        for target in re.findall(r"\]\((?!https?://|#|mailto:)([^)\s]+)\)", read(root, doc)):
            links += 1
            path = (root / doc).parent / target.split("#", 1)[0]
            if not path.exists():
                errors.append(f"{doc}: relative link '{target}' does not resolve")
    print(f"compared: index rows {len(rows)}, relative links {links}")
    if not rows or not links:
        errors.append("compared nothing (empty index or no relative links)")
    return errors


def fetch(url: str, token: str | None) -> tuple[int, bytes]:
    headers = {"User-Agent": "alice-eco-system-readme-index", "Accept": "application/vnd.github+json"}
    if token and "api.github.com" in url:
        headers["Authorization"] = f"Bearer {token}"
    try:
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as err:
        return err.code, b""


def gh(path: str, token: str | None):
    status, body = fetch(f"https://api.github.com/{path}", token)
    if status == 404:
        return None
    if status != 200:
        raise SystemExit(f"error: GitHub API {path} returned {status}")
    return json.loads(body)


def raw(repo: str, path: str, branch: str) -> str | None:
    status, body = fetch(f"https://raw.githubusercontent.com/{OWNER}/{repo}/{branch}/{path}", None)
    return body.decode("utf-8") if status == 200 else None


def crate_license(repo: str, branch: str, crate: str) -> str | None:
    text = raw(repo, "Cargo.toml", branch)
    if text is None:
        return None
    manifest = tomllib.loads(text)
    pkg = manifest.get("package", {})
    lic = pkg.get("license")
    if isinstance(lic, str):
        return lic
    ws_lic = manifest.get("workspace", {}).get("package", {}).get("license")
    if isinstance(lic, dict) or (not pkg and ws_lic and not crate):
        return ws_lic
    for member in manifest.get("workspace", {}).get("members", []):
        sub = raw(repo, f"{member}/Cargo.toml", branch)
        if sub is None:
            continue
        sub_pkg = tomllib.loads(sub).get("package", {})
        if sub_pkg.get("name") == crate:
            sub_lic = sub_pkg.get("license")
            return ws_lic if isinstance(sub_lic, dict) else sub_lic
    return ws_lic


def is_service_template(names: set[str]) -> bool:
    return {"docker-compose.yml", "database", "frontend"} <= names


def check_online(root: Path) -> list[str]:
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    errors = []
    rows = load_rows(root)
    listed = {r["repository"] for r in rows}
    compared = 0
    for row in rows:
        repo = row["repository"]
        meta = gh(f"repos/{OWNER}/{repo}", token)
        if meta is None or meta.get("private") or meta.get("archived"):
            errors.append(f"{repo}: not a public, active repository")
            continue
        names = {e["name"] for e in gh(f"repos/{OWNER}/{repo}/contents", token) or []}
        if "Cargo.toml" not in names:
            errors.append(f"{repo}: no Cargo.toml at the repository root")
        if is_service_template(names):
            errors.append(f"{repo}: hosted service template, not listed in the crate index")
        lic = crate_license(repo, meta["default_branch"], row["crate"])
        if lic != row["license"]:
            errors.append(f"{repo}: license column '{row['license']}' but Cargo.toml says '{lic}'")
        if row["crate"]:
            status, _ = fetch(f"https://crates.io/api/v1/crates/{row['crate']}", None)
            published = status == 200
            if published != (row["crates.io"] == "yes"):
                errors.append(f"{repo}: crates.io column '{row['crates.io']}' but published={published}")
        compared += 1
    # completeness: every public ALICE-* Rust crate repository is listed
    candidates = 0
    page = 1
    while True:
        batch = gh(f"users/{OWNER}/repos?type=owner&per_page=100&page={page}", token) or []
        for meta in batch:
            name = meta["name"]
            if (not name.startswith("ALICE-") or meta.get("private") or meta.get("archived")
                    or "SaaS" in name or name == SELF or name in listed):
                continue
            names = {e["name"] for e in gh(f"repos/{OWNER}/{name}/contents", token) or []}
            if "Cargo.toml" in names and not is_service_template(names):
                errors.append(f"{name}: public crate repository missing from {TSV}")
            candidates += 1
        if len(batch) < 100:
            break
        page += 1
    print(f"compared: index rows {compared}, unlisted public repositories {candidates}")
    if not compared:
        errors.append("compared nothing")
    return errors


def main() -> int:
    args = sys.argv[1:]
    root = ROOT
    if "--root" in args:
        root = Path(args[args.index("--root") + 1]).resolve()
    if "--write" in args:
        head, _, tail = split_readme(read(root, README))
        (root / README).write_text(head + render(load_rows(root)) + tail, encoding="utf-8")
        return 0
    if "--online" in args:
        errors = check_online(root)
    elif "--check" in args:
        errors = check_offline(root)
    else:
        print(__doc__)
        return 2
    for err in errors:
        print(f"error: {err}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
