#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path


def scan(path: Path) -> list[str]:
    errors: list[str] = []
    raw = path.read_bytes()
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        return [f"{path}: invalid UTF-8: {exc}"]

    if b"\r\n" in raw or b"\r" in raw:
        errors.append(f"{path}: non-LF line ending")
    if not text.endswith("\n"):
        errors.append(f"{path}: missing final newline")

    # Only SQL files use the SQL-aware delimiter scan. Markdown and JSON may
    # legitimately contain sequences such as /* in prose or paths.
    if path.suffix != ".sql":
        return errors

    state = "normal"
    i = 0
    stack: list[str] = []
    pairs = {")": "(", "]": "[", "}": "{"}

    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""

        if state == "normal":
            if ch == "'": state = "single"
            elif ch == '"': state = "double"
            elif ch == "$" and nxt == "$": state = "dollar"; i += 1
            elif ch == "-" and nxt == "-": state = "line_comment"; i += 1
            elif ch == "/" and nxt == "*": state = "block_comment"; i += 1
            elif ch in "([{": stack.append(ch)
            elif ch in ")]}":
                if not stack or stack.pop() != pairs[ch]:
                    errors.append(f"{path}: unmatched {ch} near byte {i}")
                    break
        elif state == "single":
            if ch == "'" and nxt == "'": i += 1
            elif ch == "'": state = "normal"
        elif state == "double":
            if ch == '"' and nxt == '"': i += 1
            elif ch == '"': state = "normal"
        elif state == "dollar":
            if ch == "$" and nxt == "$": state = "normal"; i += 1
        elif state == "line_comment":
            if ch == "\n": state = "normal"
        elif state == "block_comment":
            if ch == "*" and nxt == "/": state = "normal"; i += 1
        i += 1

    if state not in {"normal", "line_comment"}:
        errors.append(f"{path}: unclosed lexical state {state}")
    if stack:
        errors.append(f"{path}: unclosed delimiters {stack}")
    return errors


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    paths: set[Path] = set(root.rglob("*.sql"))
    paths.update(root.rglob("*.json"))
    paths.update(root.rglob("*.md"))
    errors: list[str] = []
    for path in sorted(paths):
        errors.extend(scan(path))

    for case in manifest["cases"]:
        for key in ("input", "expected_full", "expected_sql_only"):
            if key in case and not (root / case[key]).exists():
                errors.append(f"manifest missing file: {case[key]}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"OK: checked {len(paths)} UTF-8/LF files and all manifest paths")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
