#!/usr/bin/env python3
"""Tests for scripts/siblings.py: the path-dependency closure over sibling
manifests (dependencies and workspace members, commented lines ignored), and
an empty set failing."""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import siblings as s  # noqa: E402


def tree(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp(prefix="siblings-"))
    for rel, text in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
    return root


class Closure(unittest.TestCase):
    def test_transitive_members_and_comments(self):
        root = tree({
            "Eco/Cargo.toml": '[dependencies]\na = { path = "../A" }\n# c = { path = "../C" }\n',
            "A/Cargo.toml": '[workspace]\nmembers = ["inner"]\n',
            "A/inner/Cargo.toml": '[dependencies]\nb = { path = "../../B/crate" }\n',
            "B/crate/Cargo.toml": '[dependencies]\neco = { path = "../../Eco" }\n',
            "C/Cargo.toml": "",
        })
        seen = []
        names = s.closure(root / "Eco", root, seen.append)
        self.assertEqual(names, ["A", "B"])
        self.assertEqual(sorted(seen), ["A", "B"])

    def test_a_path_outside_the_parent_is_not_a_sibling(self):
        root = tree({"Eco/Cargo.toml": '[dependencies]\nx = { path = "/elsewhere/X" }\n'})
        self.assertEqual(s.closure(root / "Eco", root, lambda n: None), [])

    def test_nothing_found_fails(self):
        root = tree({"Eco/Cargo.toml": "[dependencies]\n", "Eco/scripts/siblings.py": ""})
        saved = s.ROOT
        try:
            s.ROOT = root / "Eco"
            self.assertEqual(s.main(["--list", "--parent", str(root)]), 1)
        finally:
            s.ROOT = saved


if __name__ == "__main__":
    unittest.main()
