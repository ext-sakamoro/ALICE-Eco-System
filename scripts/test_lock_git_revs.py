#!/usr/bin/env python3
"""Tests for scripts/lock_git_revs.py.

Each case writes a small Cargo.lock to a temporary directory and runs the check
on it. The first case runs it on this repository's own lock.
"""

from __future__ import annotations

import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lock_git_revs as lg  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SIGN = "git+https://github.com/ext-sakamoro/ALICE-Sign?rev={r}#{r}"
A = "a" * 40
B = "b" * 40


def package(name: str, source: str | None = None) -> str:
    text = f'[[package]]\nname = "{name}"\nversion = "0.1.0"\n'
    if source:
        text += f'source = "{source}"\n'
    return text + "\n"


def run(lock_text: str) -> tuple[int, str]:
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "Cargo.lock")
        with open(path, "w", encoding="utf-8") as f:
            f.write("version = 4\n\n" + lock_text)
        out = io.StringIO()
        with redirect_stdout(out):
            code = lg.main(["lock_git_revs.py", path])
        return code, out.getvalue()


class LockGitRevs(unittest.TestCase):
    def test_this_repository(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            code = lg.main(["lock_git_revs.py", os.path.join(ROOT, "Cargo.lock")])
        out = buf.getvalue()
        self.assertEqual(code, 0, out)
        self.assertNotIn("compared: alice-* git packages 0", out)

    def test_one_revision_passes(self):
        code, out = run(package("alice-sign", SIGN.format(r=A)) + package("alice-x"))
        self.assertEqual(code, 0, out)
        self.assertIn("compared: alice-* git packages 1", out)

    def test_two_revisions_fail_and_are_named(self):
        code, out = run(package("alice-sign", SIGN.format(r=A)) + package("alice-sign", SIGN.format(r=B)))
        self.assertEqual(code, 1, out)
        self.assertIn("alice-sign is linked from 2 git revisions", out)
        self.assertIn(A[:12], out)
        self.assertIn(B[:12], out)

    def test_three_revisions_fail(self):
        lock = "".join(package("alice-sign", SIGN.format(r=c * 40)) for c in "abc")
        code, out = run(lock)
        self.assertEqual(code, 1, out)
        self.assertIn("linked from 3 git revisions", out)

    def test_other_crates_and_registry_copies_are_not_counted(self):
        reg = "registry+https://github.com/rust-lang/crates.io-index"
        lock = (
            package("serde", SIGN.format(r=A))
            + package("serde", SIGN.format(r=B))
            + package("alice-crypto", reg)
            + package("alice-crypto")
            + package("alice-sign", SIGN.format(r=A))
        )
        code, out = run(lock)
        self.assertEqual(code, 0, out)
        self.assertIn("compared: alice-* git packages 1", out)

    def test_nothing_to_compare_is_exit_2(self):
        code, out = run(package("alice-x") + package("serde", "registry+https://github.com/rust-lang/crates.io-index"))
        self.assertEqual(code, 2, out)

    def test_unreadable_lock_is_exit_2(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = lg.main(["lock_git_revs.py", os.path.join(tempfile.gettempdir(), "no-such-dir", "Cargo.lock")])
        self.assertEqual(code, 2, out.getvalue())


if __name__ == "__main__":
    unittest.main()
