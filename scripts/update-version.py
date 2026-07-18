#!/usr/bin/env python3
"""Update release version references across the workspace."""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
SEMVER_RE = re.compile(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?")
DOC_VERSION_FILES = [
    ROOT / "README.md",
    ROOT / "README.ja.md",
    ROOT / "docs-site" / "src" / "index.md",
    ROOT / "docs-site" / "src" / "cli.md",
]
JSON_VERSION_FILES = [
    ROOT / "editors" / "package.json",
    ROOT / "extensions" / "chrome" / "manifest.json",
]
LOCKED_WORKSPACE_PACKAGES = {
    "sql-dialect-fmt",
    "sql-dialect-fmt-config",
    "sql-dialect-fmt-encoding",
    "sql-dialect-fmt-formatter",
    "sql-dialect-fmt-highlight",
    "sql-dialect-fmt-hover",
    "sql-dialect-fmt-lexer",
    "sql-dialect-fmt-lsp",
    "sql-dialect-fmt-parser",
    "sql-dialect-fmt-syntax",
    "sql-dialect-fmt-test-fixtures",
    "sql-dialect-fmt-test-support",
    "sql-dialect-fmt-text",
    "sql-dialect-fmt-tree-sitter",
    "sql-dialect-fmt-wasm",
}


def normalize_version(raw: str) -> str:
    version = raw[1:] if raw.startswith("v") else raw
    if not SEMVER_RE.fullmatch(version):
        raise argparse.ArgumentTypeError(
            f"version must look like v1.2.3 or 1.2.3, got {raw!r}"
        )
    return version


def workspace_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["workspace"]["package"]["version"]


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def write_if_changed(
    path: pathlib.Path,
    text: str,
    changed: list[pathlib.Path],
    check: bool,
) -> None:
    old = read(path)
    if old == text:
        return
    changed.append(path)
    if not check:
        path.write_text(text, encoding="utf-8")


def replace_exact(path: pathlib.Path, old: str, new: str, check: bool) -> bool:
    text = read(path)
    updated = text.replace(f"v{old}", f"v{new}").replace(old, new)
    if updated == text:
        return False
    if not check:
        path.write_text(updated, encoding="utf-8")
    return True


def update_cargo_toml(old: str, new: str, changed: list[pathlib.Path], check: bool) -> None:
    path = ROOT / "Cargo.toml"
    text = read(path)
    updated = re.sub(rf'version = "{re.escape(old)}"', f'version = "{new}"', text)
    write_if_changed(path, updated, changed, check)


def update_json_versions(new: str, changed: list[pathlib.Path], check: bool) -> None:
    for path in JSON_VERSION_FILES:
        text = read(path)
        updated, count = re.subn(
            r'("version"\s*:\s*")[^"]+(")',
            rf"\g<1>{new}\2",
            text,
            count=1,
        )
        if count != 1:
            raise ValueError(f"{path.relative_to(ROOT)} must contain one top-level version field")
        write_if_changed(path, updated, changed, check)


def update_formula(new: str, changed: list[pathlib.Path], check: bool) -> None:
    path = ROOT / "Formula" / "sql-dialect-fmt.rb"
    text = read(path)
    updated, count = re.subn(r'tag: "v[^"]+"', f'tag: "v{new}"', text, count=1)
    if count != 1:
        raise ValueError(f"{path.relative_to(ROOT)} must pin one v-prefixed git tag")
    write_if_changed(path, updated, changed, check)


def update_docs(old: str, new: str, changed: list[pathlib.Path], check: bool) -> None:
    for path in DOC_VERSION_FILES:
        if replace_exact(path, old, new, check):
            changed.append(path)


def update_cargo_lock(new: str, changed: list[pathlib.Path], check: bool) -> None:
    path = ROOT / "Cargo.lock"
    text = read(path)
    parts = re.split(r"(?m)(?=^\[\[package\]\]\n)", text)
    updated_parts: list[str] = []
    for part in parts:
        name_match = re.search(r'(?m)^name = "([^"]+)"$', part)
        if name_match and name_match.group(1) in LOCKED_WORKSPACE_PACKAGES:
            part = re.sub(r'(?m)^version = "[^"]+"$', f'version = "{new}"', part, count=1)
        updated_parts.append(part)
    write_if_changed(path, "".join(updated_parts), changed, check)


def update_changelog(
    old: str,
    new: str,
    date: str,
    changed: list[pathlib.Path],
    check: bool,
) -> None:
    path = ROOT / "CHANGELOG.md"
    text = read(path)
    if f"## [{new}]" not in text:
        text = text.replace("## [Unreleased]\n", f"## [Unreleased]\n\n## [{new}] - {date}\n", 1)
    text = re.sub(
        rf"\[Unreleased\]: https://github\.com/hjosugi/sql-dialect-fmt/compare/v{re.escape(old)}\.\.\.HEAD",
        f"[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v{new}...HEAD",
        text,
        count=1,
    )
    new_link = f"[{new}]: https://github.com/hjosugi/sql-dialect-fmt/compare/v{old}...v{new}"
    if new_link not in text:
        text = text.replace(
            f"[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v{new}...HEAD\n",
            f"[Unreleased]: https://github.com/hjosugi/sql-dialect-fmt/compare/v{new}...HEAD\n{new_link}\n",
            1,
        )
    write_if_changed(path, text, changed, check)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Update workspace release version references in place.",
    )
    parser.add_argument("version", type=normalize_version, help="new version, e.g. 1.2.4 or v1.2.4")
    parser.add_argument(
        "--check",
        action="store_true",
        help="report files that would change without writing them",
    )
    parser.add_argument(
        "--changelog",
        action="store_true",
        help="also create/update the CHANGELOG.md release section and compare links",
    )
    parser.add_argument(
        "--date",
        default=dt.date.today().isoformat(),
        help="release date for --changelog (default: today)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    old = workspace_version()
    new = args.version
    changed: list[pathlib.Path] = []

    update_cargo_toml(old, new, changed, args.check)
    update_cargo_lock(new, changed, args.check)
    update_json_versions(new, changed, args.check)
    update_formula(new, changed, args.check)
    update_docs(old, new, changed, args.check)
    if args.changelog:
        update_changelog(old, new, args.date, changed, args.check)

    if args.check and changed:
        print("version update needed:", file=sys.stderr)
        for path in changed:
            print(f"  {path.relative_to(ROOT)}", file=sys.stderr)
        return 1

    action = "would update" if args.check else "updated"
    if changed:
        print(f"{action} {len(changed)} file(s) to {new}:")
        for path in changed:
            print(f"  {path.relative_to(ROOT)}")
    else:
        print(f"version references already at {new}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
