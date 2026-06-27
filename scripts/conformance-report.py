#!/usr/bin/env python3
"""Generate a conformance report from docs/examples and run the corpus harness.

The generator intentionally stays conservative: it collects committed ``*.sql`` files and SQL-ish
fenced code blocks, writes them to a temporary corpus directory, and delegates invariants to the
same Rust harness as CI. That gives ROADMAP-visible parser-gap reports without introducing a second
validation engine.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SQL_FENCE_RE = re.compile(
    r"```(?:sql|snowflake|snowflake-sql|snowsql)\b[^\n]*\n(.*?)\n```",
    re.IGNORECASE | re.DOTALL,
)
TEXT_EXTENSIONS = {".md", ".markdown", ".rst", ".html", ".htm"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--path", help="Local directory, SQL file, or archive to mine.")
    source.add_argument("--url", help="Archive URL to download and mine.")
    parser.add_argument(
        "--out",
        default="target/conformance-report.md",
        help="Markdown report path. Defaults to target/conformance-report.md.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=int(os.environ.get("SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT", "0") or "0"),
        help="Optional file cap passed to the corpus harness.",
    )
    parser.add_argument(
        "--keep-workdir",
        action="store_true",
        help="Keep the mined corpus directory for debugging.",
    )
    return parser.parse_args()


def download(url: str, workdir: Path) -> Path:
    archive = workdir / "source.archive"
    with urllib.request.urlopen(url) as response, archive.open("wb") as out:
        shutil.copyfileobj(response, out)
    return archive


def unpack_if_needed(path: Path, workdir: Path) -> Path:
    if path.is_dir():
        return path
    if path.suffix.lower() == ".sql":
        single = workdir / "single"
        single.mkdir()
        shutil.copy2(path, single / path.name)
        return single

    extracted = workdir / "extracted"
    extracted.mkdir()
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            for member in archive.infolist():
                target = (extracted / member.filename).resolve()
                if not str(target).startswith(str(extracted.resolve())):
                    raise SystemExit(f"unsafe archive path: {member.filename}")
            archive.extractall(extracted)
        return extracted
    if tarfile.is_tarfile(path):
        with tarfile.open(path) as archive:
            for member in archive.getmembers():
                target = (extracted / member.name).resolve()
                if not str(target).startswith(str(extracted.resolve())):
                    raise SystemExit(f"unsafe archive path: {member.name}")
            archive.extractall(extracted)
        return extracted
    raise SystemExit(f"unsupported source path: {path}")


def safe_name(path: Path) -> str:
    return "__".join(part for part in path.parts if part not in ("", os.sep))


def collect_sql_files(source: Path, corpus: Path) -> list[Path]:
    files: list[Path] = []
    for path in sorted(source.rglob("*")):
        if path.is_file() and path.suffix.lower() == ".sql":
            rel = path.relative_to(source)
            target = corpus / "sql_files" / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            files.append(target)
    return files


def collect_fenced_sql(source: Path, corpus: Path) -> list[Path]:
    files: list[Path] = []
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in TEXT_EXTENSIONS:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for index, match in enumerate(SQL_FENCE_RE.finditer(text), start=1):
            sql = match.group(1).strip()
            if not sql:
                continue
            rel = path.relative_to(source)
            target = corpus / "fenced_sql" / f"{safe_name(rel)}.{index:03}.sql"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(sql + "\n", encoding="utf-8")
            files.append(target)
    return files


def run_harness(corpus: Path, limit: int) -> subprocess.CompletedProcess[str]:
    command = [str(ROOT / "scripts" / "run-external-corpus.sh"), "--path", str(corpus)]
    if limit > 0:
        command.extend(["--limit", str(limit)])
    return subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def write_report(
    out: Path,
    source_label: str,
    corpus: Path,
    sql_files: list[Path],
    fenced_files: list[Path],
    result: subprocess.CompletedProcess[str] | None,
    limit: int,
) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    mined = sql_files + fenced_files
    status = "not run" if result is None else ("passed" if result.returncode == 0 else "failed")
    lines = [
        "# Conformance Report",
        "",
        f"- Source: `{source_label}`",
        f"- Mined corpus: `{corpus}`",
        f"- SQL files: {len(sql_files)}",
        f"- SQL fenced blocks: {len(fenced_files)}",
        f"- Harness limit: {limit if limit > 0 else 'none'}",
        f"- Harness status: {status}",
        "",
        "## Sample Files",
        "",
    ]
    for path in mined[:30]:
        lines.append(f"- `{path.relative_to(corpus)}`")
    if len(mined) > 30:
        lines.append(f"- ... {len(mined) - 30} more")
    if result is not None:
        lines.extend(
            [
                "",
                "## Harness Output",
                "",
                "```text",
                result.stdout.rstrip(),
                "```",
            ]
        )
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    workdir = Path(tempfile.mkdtemp(prefix="sql-dialect-fmt-conformance-"))
    keep_workdir = args.keep_workdir
    try:
        if args.url:
            source_path = unpack_if_needed(download(args.url, workdir), workdir)
            source_label = args.url
        else:
            source_path = unpack_if_needed(Path(args.path).resolve(), workdir)
            source_label = str(Path(args.path).resolve())

        corpus = workdir / "corpus"
        corpus.mkdir()
        sql_files = collect_sql_files(source_path, corpus)
        fenced_files = collect_fenced_sql(source_path, corpus)
        mined_count = len(sql_files) + len(fenced_files)

        result = None
        if mined_count > 0:
            result = run_harness(corpus, args.limit)

        out = (ROOT / args.out).resolve() if not Path(args.out).is_absolute() else Path(args.out)
        write_report(out, source_label, corpus, sql_files, fenced_files, result, args.limit)
        print(f"conformance report written to {out}")

        if args.keep_workdir:
            print(f"kept mined corpus at {workdir}")
        elif result is not None and result.returncode != 0:
            print(f"failed mined corpus kept at {workdir}", file=sys.stderr)
            keep_workdir = True

        if mined_count == 0:
            print("no SQL files or SQL fenced blocks were mined", file=sys.stderr)
            return 1
        return result.returncode if result is not None else 1
    finally:
        if not keep_workdir:
            shutil.rmtree(workdir, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
