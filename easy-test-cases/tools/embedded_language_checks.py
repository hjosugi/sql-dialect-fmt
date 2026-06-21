#!/usr/bin/env python3
from __future__ import annotations

import argparse
import ast
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

LANGUAGE_PATTERN = re.compile(r"LANGUAGE\s+(JAVASCRIPT|PYTHON)\b", re.IGNORECASE)


def extract_bodies(path: Path) -> list[tuple[str, str]]:
    text = path.read_text(encoding="utf-8")
    bodies: list[tuple[str, str]] = []
    for match in LANGUAGE_PATTERN.finditer(text):
        language = match.group(1).upper()
        start = text.find("$$", match.end())
        if start < 0:
            raise ValueError(f"{path}: missing opening $$ for {language} body")
        end = text.find("$$", start + 2)
        if end < 0:
            raise ValueError(f"{path}: missing closing $$ for {language} body")
        bodies.append((language, text[start + 2 : end].strip("\n")))
    return bodies


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--require-node",
        action="store_true",
        help="Fail rather than skip JavaScript syntax checks when Node.js is unavailable.",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    paths = sorted(root.rglob("*.sql"))
    python_count = 0
    javascript_count = 0
    node = shutil.which("node")

    if args.require_node and node is None:
        print("FAIL: Node.js is required but was not found")
        return 1

    with tempfile.TemporaryDirectory(prefix="sf-embedded-") as tmp:
        tmpdir = Path(tmp)
        js_index = 0

        for path in paths:
            for language, body in extract_bodies(path):
                if language == "PYTHON":
                    try:
                        ast.parse(body, filename=str(path))
                    except SyntaxError as exc:
                        print(f"FAIL Python body in {path}: {exc}")
                        return 1
                    python_count += 1
                elif language == "JAVASCRIPT":
                    javascript_count += 1
                    if node is not None:
                        js_path = tmpdir / f"body_{js_index}.js"
                        js_index += 1
                        js_path.write_text(body + "\n", encoding="utf-8", newline="\n")
                        result = subprocess.run(
                            [node, "--check", str(js_path)],
                            text=True,
                            capture_output=True,
                        )
                        if result.returncode != 0:
                            print(f"FAIL JavaScript body in {path}")
                            print(result.stdout)
                            print(result.stderr)
                            return 1

    js_status = "checked with Node.js" if node is not None else "counted; Node.js unavailable"
    print(
        f"OK: {python_count} Python bodies parsed; "
        f"{javascript_count} JavaScript bodies {js_status}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
