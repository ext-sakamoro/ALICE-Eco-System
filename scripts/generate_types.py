#!/usr/bin/env python3
"""ALICE-Eco-System ブリッジ構造体 → Python dataclass / TypeScript interface 生成

使い方:
    python3 scripts/generate_types.py               # stdout に出力
    python3 scripts/generate_types.py --python       # Python のみ
    python3 scripts/generate_types.py --typescript    # TypeScript のみ
    python3 scripts/generate_types.py -o out/         # ファイル出力

対象: src/bridge_*.rs 内の `pub struct` を解析し型定義を生成。
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

# Rust → Python 型マッピング
RUST_TO_PYTHON: dict[str, str] = {
    "u8": "int",
    "u16": "int",
    "u32": "int",
    "u64": "int",
    "u128": "int",
    "usize": "int",
    "i8": "int",
    "i16": "int",
    "i32": "int",
    "i64": "int",
    "i128": "int",
    "isize": "int",
    "f32": "float",
    "f64": "float",
    "bool": "bool",
    "String": "str",
}

# Rust → TypeScript 型マッピング
RUST_TO_TS: dict[str, str] = {
    "u8": "number",
    "u16": "number",
    "u32": "number",
    "u64": "bigint",
    "u128": "bigint",
    "usize": "number",
    "i8": "number",
    "i16": "number",
    "i32": "number",
    "i64": "bigint",
    "i128": "bigint",
    "isize": "number",
    "f32": "number",
    "f64": "number",
    "bool": "boolean",
    "String": "string",
}


@dataclass
class Field:
    name: str
    rust_type: str
    doc: str


@dataclass
class BridgeStruct:
    name: str
    doc: str
    fields: list[Field]
    source_file: str


# ── パーサー ────────────────────────────────────────────────────────────

# 固定長配列パターン: [T; N]
RE_ARRAY = re.compile(r"^\[(\w+);\s*(\d+)\]$")


def parse_bridge_structs(src_dir: Path) -> list[BridgeStruct]:
    """src/bridge_*.rs から pub struct とそのフィールドを抽出。"""
    structs: list[BridgeStruct] = []

    for path in sorted(src_dir.glob("bridge_*.rs")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        i = 0
        while i < len(lines):
            line = lines[i]

            # pub struct 検出
            m = re.match(r"^pub struct (\w+)\s*\{", line)
            if not m:
                i += 1
                continue

            struct_name = m.group(1)

            # 直前のドキュメントコメントを収集
            doc_lines: list[str] = []
            j = i - 1
            while j >= 0 and lines[j].strip().startswith("///"):
                doc_lines.insert(0, lines[j].strip().lstrip("/ "))
                j -= 1

            # フィールドを解析
            fields: list[Field] = []
            i += 1
            field_doc: list[str] = []
            while i < len(lines):
                fl = lines[i].strip()
                if fl == "}":
                    break
                if fl.startswith("///"):
                    field_doc.append(fl.lstrip("/ "))
                    i += 1
                    continue
                # pub field: Type,
                fm = re.match(r"pub\s+(\w+)\s*:\s*(.+?)\s*,?$", fl)
                if fm:
                    fname = fm.group(1)
                    ftype = fm.group(2).rstrip(",").strip()
                    fields.append(Field(
                        name=fname,
                        rust_type=ftype,
                        doc=" ".join(field_doc).strip(),
                    ))
                    field_doc = []
                i += 1

            if fields:
                structs.append(BridgeStruct(
                    name=struct_name,
                    doc=" ".join(doc_lines).strip(),
                    fields=fields,
                    source_file=path.name,
                ))
            i += 1

    return structs


# ── Python 出力 ─────────────────────────────────────────────────────────

def rust_type_to_python(rt: str) -> str:
    if rt in RUST_TO_PYTHON:
        return RUST_TO_PYTHON[rt]
    m = RE_ARRAY.match(rt)
    if m:
        inner = RUST_TO_PYTHON.get(m.group(1), "Any")
        return f"list[{inner}]"
    return "Any"


def generate_python(structs: list[BridgeStruct]) -> str:
    lines = [
        '"""ALICE-Eco-System ブリッジ型定義 (自動生成)"""',
        "",
        "from __future__ import annotations",
        "",
        "from dataclasses import dataclass",
        "from typing import Any",
        "",
    ]

    for s in structs:
        lines.append("")
        lines.append(f"# source: {s.source_file}")
        lines.append("@dataclass")
        lines.append(f"class {s.name}:")
        if s.doc:
            lines.append(f'    """{s.doc}"""')
        for f in s.fields:
            py_type = rust_type_to_python(f.rust_type)
            if f.doc:
                lines.append(f"    # {f.doc}")
            lines.append(f"    {f.name}: {py_type}")
        lines.append("")

    return "\n".join(lines)


# ── TypeScript 出力 ─────────────────────────────────────────────────────

def rust_type_to_ts(rt: str) -> str:
    if rt in RUST_TO_TS:
        return RUST_TO_TS[rt]
    m = RE_ARRAY.match(rt)
    if m:
        inner = RUST_TO_TS.get(m.group(1), "unknown")
        n = int(m.group(2))
        # 固定長タプル
        return f"[{', '.join([inner] * n)}]"
    return "unknown"


def generate_typescript(structs: list[BridgeStruct]) -> str:
    lines = [
        "// ALICE-Eco-System ブリッジ型定義 (自動生成)",
        "",
    ]

    for s in structs:
        lines.append(f"// source: {s.source_file}")
        if s.doc:
            lines.append(f"/** {s.doc} */")
        lines.append(f"export interface {s.name} {{")
        for f in s.fields:
            ts_type = rust_type_to_ts(f.rust_type)
            if f.doc:
                lines.append(f"  /** {f.doc} */")
            lines.append(f"  {f.name}: {ts_type};")
        lines.append("}")
        lines.append("")

    return "\n".join(lines)


# ── main ────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description="ALICE bridge struct → Python/TS type generator")
    parser.add_argument("--python", action="store_true", help="Python のみ出力")
    parser.add_argument("--typescript", action="store_true", help="TypeScript のみ出力")
    parser.add_argument("-o", "--outdir", type=str, help="出力ディレクトリ（省略時は stdout）")
    args = parser.parse_args()

    # デフォルト: 両方出力
    do_python = args.python or (not args.python and not args.typescript)
    do_ts = args.typescript or (not args.python and not args.typescript)

    src_dir = Path(__file__).resolve().parent.parent / "src"
    if not src_dir.exists():
        print(f"Error: src directory not found at {src_dir}", file=sys.stderr)
        sys.exit(1)

    structs = parse_bridge_structs(src_dir)
    print(f"# Parsed {len(structs)} bridge structs from {src_dir}", file=sys.stderr)

    if args.outdir:
        os.makedirs(args.outdir, exist_ok=True)
        if do_python:
            out_py = Path(args.outdir) / "alice_bridges.py"
            out_py.write_text(generate_python(structs), encoding="utf-8")
            print(f"  → {out_py} ({len(structs)} classes)", file=sys.stderr)
        if do_ts:
            out_ts = Path(args.outdir) / "alice_bridges.ts"
            out_ts.write_text(generate_typescript(structs), encoding="utf-8")
            print(f"  → {out_ts} ({len(structs)} interfaces)", file=sys.stderr)
    else:
        if do_python:
            print(generate_python(structs))
        if do_ts:
            print(generate_typescript(structs))


if __name__ == "__main__":
    main()
