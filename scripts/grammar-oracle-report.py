#!/usr/bin/env python3
"""Compare upstream grammar oracles with the handwritten Rust parser inventory.

This script deliberately does not generate parser code. It extracts stable, machine-readable
checklists from three independent upstream sources:

* grammars-v4 Snowflake examples and parser rules;
* Apache Spark SQL tests and ``SqlBaseParser.g4`` rules;
* sqlfluff Snowflake/Databricks keyword and dialect-specific segment inventories.

The resulting Markdown report is a drift signal, not a claim of semantic parser coverage. Optional
corpus runs reuse ``conformance-report.py`` and the lossless/idempotent formatter harness.
"""

from __future__ import annotations

import argparse
import ast
import difflib
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[1]

GRAMMARS_V4_MARKER = Path("sql/snowflake/SnowflakeParser.g4")
SPARK_MARKER = Path(
    "sql/api/src/main/antlr4/org/apache/spark/sql/catalyst/parser/SqlBaseParser.g4"
)
SQLFLUFF_MARKER = Path("src/sqlfluff/dialects/dialect_snowflake.py")

GRAMMARS_V4_URL = "https://github.com/antlr/grammars-v4/tree/master/sql/snowflake"
SPARK_URL = (
    "https://github.com/apache/spark/tree/master/sql/api/src/main/antlr4/"
    "org/apache/spark/sql/catalyst/parser"
)
SQLFLUFF_URL = "https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/dialects"


@dataclass(frozen=True)
class Source:
    name: str
    root: Path
    revision: str
    url: str
    license: str


@dataclass(frozen=True)
class Match:
    upstream: str
    normalized: str
    status: str
    local: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--grammars-v4", required=True, help="grammars-v4 checkout/archive root")
    parser.add_argument("--spark", required=True, help="Apache Spark checkout/archive root")
    parser.add_argument("--sqlfluff", required=True, help="sqlfluff checkout/archive root")
    parser.add_argument("--grammars-v4-revision", help="grammars-v4 commit/tag for attribution")
    parser.add_argument("--spark-revision", help="Spark commit/tag for attribution")
    parser.add_argument("--sqlfluff-revision", help="sqlfluff commit/tag for attribution")
    parser.add_argument(
        "--out",
        default="target/grammar-oracle-report.md",
        help="Markdown output path. Defaults to target/grammar-oracle-report.md.",
    )
    parser.add_argument(
        "--run-corpora",
        action="store_true",
        help="Run grammars-v4 examples as Snowflake and Spark SQL tests as Databricks.",
    )
    parser.add_argument(
        "--corpus-limit",
        type=int,
        default=0,
        help="Optional per-corpus SQL file cap when --run-corpora is enabled.",
    )
    return parser.parse_args()


def locate_root(path: str, marker: Path) -> Path:
    candidate = Path(path).resolve()
    if (candidate / marker).is_file():
        return candidate
    if candidate.is_dir():
        matches = sorted(
            child for child in candidate.iterdir() if child.is_dir() and (child / marker).is_file()
        )
        if len(matches) == 1:
            return matches[0]
    raise SystemExit(f"{candidate} does not contain {marker}")


def git_revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else "not provided"


def source(
    name: str,
    path: str,
    marker: Path,
    revision: str | None,
    url: str,
    license_name: str,
) -> Source:
    root = locate_root(path, marker)
    return Source(name, root, revision or git_revision(root), url, license_name)


def without_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def extract_antlr_rules(text: str) -> list[str]:
    """Return parser-rule names without evaluating embedded target-language actions."""
    cleaned = without_comments(text)
    pattern = re.compile(
        r"(?m)^[ \t]*([a-z][A-Za-z0-9_]*)"
        r"(?:\s*\[[^\]\n]*\])?"
        r"(?:\s+returns\s*\[[^\]\n]*\])?"
        r"(?:\s+locals\s*\[[^\]\n]*\])?"
        r"\s*:"
    )
    return sorted(set(pattern.findall(cleaned)))


def literal_words(value: ast.AST) -> set[str]:
    if isinstance(value, ast.Constant) and isinstance(value.value, str):
        return {word.upper() for word in value.value.split() if word}
    if isinstance(value, (ast.List, ast.Tuple, ast.Set)):
        words = set()
        for item in value.elts:
            if isinstance(item, ast.Constant) and isinstance(item.value, str):
                words.add(item.value.upper())
        return words
    return set()


def extract_python_word_assignments(text: str) -> dict[str, set[str]]:
    tree = ast.parse(text)
    assignments: dict[str, set[str]] = {}
    for node in tree.body:
        if not isinstance(node, (ast.Assign, ast.AnnAssign)):
            continue
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        value = node.value
        if value is None:
            continue
        words = literal_words(value)
        if not words:
            continue
        for target in targets:
            if isinstance(target, ast.Name):
                assignments[target.id] = words
    return assignments


def extract_python_segments(text: str) -> list[str]:
    tree = ast.parse(text)
    return sorted(
        {
            node.name
            for node in ast.walk(tree)
            if isinstance(node, ast.ClassDef) and node.name.endswith("Segment")
        }
    )


def camel_to_snake(name: str) -> str:
    step = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", step).lower()


def normalize_symbol(name: str) -> str:
    normalized = camel_to_snake(name)
    normalized = re.sub(r"_segment$", "", normalized)
    normalized = normalized.replace("_statement", "_stmt")
    normalized = normalized.replace("_expression", "_expr")
    normalized = normalized.replace("_identifier", "_name")
    return normalized


def extract_local_inventory(root: Path) -> tuple[dict[str, str], set[str], set[str]]:
    keyword_path = root / "crates/sql-dialect-fmt-syntax/src/keyword.rs"
    contextual_path = root / "crates/sql-dialect-fmt-parser/src/contextual.rs"
    kind_path = root / "crates/sql-dialect-fmt-syntax/src/kind.rs"
    grammar_root = root / "crates/sql-dialect-fmt-parser/src/grammar"

    keyword_text = keyword_path.read_text(encoding="utf-8")
    keywords = {
        word.upper(): dialect
        for word, dialect in re.findall(
            r'\("([a-z0-9_]+)",\s*[A-Z0-9_]+_KW,\s*'
            r"(Shared|SnowflakeOnly|DatabricksOnly)\)",
            keyword_text,
        )
    }
    contextual = {
        word.upper()
        for word in re.findall(
            r'^\s*[A-Za-z0-9_]+\s*=>\s*"([a-z0-9_]+)",',
            contextual_path.read_text(encoding="utf-8"),
            flags=re.MULTILINE,
        )
    }

    grammar_functions = set()
    for path in sorted(grammar_root.rglob("*.rs")):
        grammar_functions.update(
            re.findall(
                r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z][a-z0-9_]*)\s*\(",
                path.read_text(encoding="utf-8"),
            )
        )

    kind_text = kind_path.read_text(encoding="utf-8")
    node_block_match = re.search(
        r"__NODE_START.*?(.*?)__NODE_END", kind_text, flags=re.DOTALL
    )
    node_kinds = (
        set(re.findall(r"(?m)^\s*([A-Z][A-Z0-9_]+),", node_block_match.group(1)))
        if node_block_match
        else set()
    )
    local_symbols = {normalize_symbol(name) for name in grammar_functions}
    local_symbols.update(normalize_symbol(name) for name in node_kinds)
    return keywords, contextual, local_symbols


def inventory_sqlfluff(root: Path) -> dict[str, set[str] | list[str]]:
    dialects = root / "src/sqlfluff/dialects"
    snowflake_words = extract_python_word_assignments(
        (dialects / "dialect_snowflake_keywords.py").read_text(encoding="utf-8")
    )
    databricks_words = extract_python_word_assignments(
        (dialects / "dialect_databricks_keywords.py").read_text(encoding="utf-8")
    )
    spark_words = extract_python_word_assignments(
        (dialects / "dialect_sparksql_keywords.py").read_text(encoding="utf-8")
    )

    snowflake_reserved = snowflake_words.get("snowflake_reserved_keywords", set())
    snowflake_unreserved = snowflake_words.get("snowflake_unreserved_keywords", set())
    databricks_reserved = databricks_words.get("RESERVED_KEYWORDS", set())
    databricks_specific_unreserved = databricks_words.get("UNRESERVED_KEYWORDS", set())
    spark_reserved = spark_words.get("RESERVED_KEYWORDS", set())
    spark_unreserved = spark_words.get("UNRESERVED_KEYWORDS", set())
    databricks_unreserved = (
        spark_reserved | spark_unreserved | databricks_specific_unreserved
    ) - databricks_reserved

    return {
        "snowflake_reserved": snowflake_reserved,
        "snowflake_unreserved": snowflake_unreserved,
        "databricks_reserved": databricks_reserved,
        "databricks_unreserved": databricks_unreserved,
        "snowflake_segments": extract_python_segments(
            (dialects / "dialect_snowflake.py").read_text(encoding="utf-8")
        ),
        "databricks_segments": extract_python_segments(
            (dialects / "dialect_databricks.py").read_text(encoding="utf-8")
        ),
    }


def match_inventory(upstream: Iterable[str], local_symbols: set[str]) -> list[Match]:
    matches = []
    for name in sorted(set(upstream)):
        normalized = normalize_symbol(name)
        if normalized in local_symbols:
            matches.append(Match(name, normalized, "name match", normalized))
            continue
        nearest = difflib.get_close_matches(normalized, local_symbols, n=1, cutoff=0.82)
        if nearest:
            matches.append(Match(name, normalized, "near name", nearest[0]))
        else:
            matches.append(Match(name, normalized, "unmatched", ""))
    return matches


def words_section(title: str, words: set[str]) -> list[str]:
    lines = [f"### {title}", "", f"Count: **{len(words)}**", ""]
    if words:
        lines.extend(f"- `{word}`" for word in sorted(words))
    else:
        lines.append("- None")
    lines.append("")
    return lines


def match_table(title: str, matches: list[Match]) -> list[str]:
    status_counts = {
        status: sum(match.status == status for match in matches)
        for status in ("name match", "near name", "unmatched")
    }
    lines = [
        f"### {title}",
        "",
        (
            f"Total **{len(matches)}**; name match **{status_counts['name match']}**; "
            f"near name **{status_counts['near name']}**; unmatched "
            f"**{status_counts['unmatched']}**."
        ),
        "",
        "| Upstream | Normalized | Name signal | Local candidate |",
        "| --- | --- | --- | --- |",
    ]
    lines.extend(
        f"| `{match.upstream}` | `{match.normalized}` | {match.status} | "
        f"{f'`{match.local}`' if match.local else '—'} |"
        for match in matches
    )
    lines.append("")
    return lines


def run_conformance(
    path: Path,
    dialect: str,
    out: Path,
    source_info: Source,
    limit: int,
) -> subprocess.CompletedProcess[str]:
    command = [
        sys.executable,
        str(ROOT / "scripts/conformance-report.py"),
        "--path",
        str(path),
        "--dialect",
        dialect,
        "--out",
        str(out),
        "--source-url",
        source_info.url,
        "--source-revision",
        source_info.revision,
        "--source-license",
        source_info.license,
    ]
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


def count_sql(path: Path) -> int:
    return sum(
        candidate.is_file() and candidate.suffix.lower() == ".sql"
        for candidate in path.rglob("*")
    )


def write_report(
    out: Path,
    grammars_v4: Source,
    spark: Source,
    sqlfluff: Source,
    run_corpora: bool,
    corpus_limit: int,
) -> int:
    local_keywords, contextual, local_symbols = extract_local_inventory(ROOT)

    snowflake_parser = (
        grammars_v4.root / GRAMMARS_V4_MARKER
    ).read_text(encoding="utf-8")
    spark_parser = (spark.root / SPARK_MARKER).read_text(encoding="utf-8")
    grammars_v4_rules = extract_antlr_rules(snowflake_parser)
    spark_rules = extract_antlr_rules(spark_parser)
    sqlfluff_inventory = inventory_sqlfluff(sqlfluff.root)

    local_snowflake_reserved = {
        word
        for word, dialect in local_keywords.items()
        if dialect in {"Shared", "SnowflakeOnly"}
    }
    local_databricks_reserved = {
        word
        for word, dialect in local_keywords.items()
        if dialect in {"Shared", "DatabricksOnly"}
    }
    local_all_words = set(local_keywords) | contextual

    sf_reserved = set(sqlfluff_inventory["snowflake_reserved"])
    sf_unreserved = set(sqlfluff_inventory["snowflake_unreserved"])
    db_reserved = set(sqlfluff_inventory["databricks_reserved"])
    db_unreserved = set(sqlfluff_inventory["databricks_unreserved"])

    snowflake_examples = grammars_v4.root / "sql/snowflake/examples"
    spark_tests = spark.root / "sql/core/src/test/resources/sql-tests/inputs"
    if not snowflake_examples.is_dir():
        raise SystemExit(f"missing Snowflake examples: {snowflake_examples}")
    if not spark_tests.is_dir():
        raise SystemExit(f"missing Spark SQL tests: {spark_tests}")

    lines = [
        "# External Grammar Oracle Report",
        "",
        "> This is a conformance checklist, not generated-parser input and not proof of semantic "
        "coverage. Name matches are heuristic review signals only.",
        "",
        "## Provenance",
        "",
        "| Source | Revision | License | Canonical location |",
        "| --- | --- | --- | --- |",
        f"| grammars-v4 Snowflake | `{grammars_v4.revision}` | {grammars_v4.license} | "
        f"[source]({grammars_v4.url}) |",
        f"| Apache Spark SQL | `{spark.revision}` | {spark.license} | "
        f"[source]({spark.url}) |",
        f"| sqlfluff dialects | `{sqlfluff.revision}` | {sqlfluff.license} | "
        f"[source]({sqlfluff.url}) |",
        "",
        "## Corpus Oracles",
        "",
        f"- grammars-v4 Snowflake examples: **{count_sql(snowflake_examples)}** SQL files",
        f"- Apache Spark SQL test inputs: **{count_sql(spark_tests)}** SQL files",
        f"- Harness execution: **{'enabled' if run_corpora else 'not requested'}**",
        "",
        "## SQLFluff Keyword Delta",
        "",
        "Reservation differences are review candidates, not automatic defects: this parser keeps "
        "many words contextual to avoid stealing valid identifiers.",
        "",
    ]
    lines.extend(
        words_section(
            "Snowflake reserved upstream but not locally reserved",
            sf_reserved - local_snowflake_reserved,
        )
    )
    lines.extend(
        words_section(
            "Snowflake keyword candidates absent from local reserved/contextual inventories",
            (sf_reserved | sf_unreserved) - local_all_words,
        )
    )
    lines.extend(
        words_section(
            "Local Snowflake reserved words absent from SQLFluff Snowflake lists",
            local_snowflake_reserved - (sf_reserved | sf_unreserved),
        )
    )
    lines.extend(
        words_section(
            "Databricks reserved upstream but not locally reserved",
            db_reserved - local_databricks_reserved,
        )
    )
    lines.extend(
        words_section(
            "Databricks keyword candidates absent from local reserved/contextual inventories",
            (db_reserved | db_unreserved) - local_all_words,
        )
    )

    lines.extend(
        match_table(
            "SQLFluff Snowflake dialect-specific segments",
            match_inventory(
                list(sqlfluff_inventory["snowflake_segments"]), local_symbols
            ),
        )
    )
    lines.extend(
        match_table(
            "SQLFluff Databricks dialect-specific segments",
            match_inventory(
                list(sqlfluff_inventory["databricks_segments"]), local_symbols
            ),
        )
    )
    lines.extend(
        match_table(
            "grammars-v4 Snowflake parser rules",
            match_inventory(grammars_v4_rules, local_symbols),
        )
    )
    lines.extend(
        [
            "## Spark / Databricks Coverage Checklist",
            "",
            "The complete `SqlBaseParser.g4` rule inventory is retained below so changes in Spark "
            "become a reviewable checklist for `grammar/delta.rs` and the shared grammar modules.",
            "",
        ]
    )
    lines.extend(match_table("Spark parser rules", match_inventory(spark_rules, local_symbols)))

    status = 0
    if run_corpora:
        out.parent.mkdir(parents=True, exist_ok=True)
        corpus_runs = [
            (
                snowflake_examples,
                "snowflake",
                out.parent / "grammars-v4-snowflake-conformance.md",
                grammars_v4,
            ),
            (
                spark_tests,
                "databricks",
                out.parent / "spark-databricks-conformance.md",
                spark,
            ),
        ]
        run_lines = ["## Corpus Run Results", ""]
        for path, dialect, report, source_info in corpus_runs:
            result = run_conformance(path, dialect, report, source_info, corpus_limit)
            state = "passed" if result.returncode == 0 else "failed"
            run_lines.append(
                f"- `{source_info.name}` as `{dialect}`: **{state}** "
                f"([report]({report.name}))"
            )
            if result.stdout.strip():
                run_lines.extend(["", "  ```text", result.stdout.rstrip(), "  ```"])
            status = max(status, result.returncode)
        run_lines.append("")
        lines[lines.index("## SQLFluff Keyword Delta"):lines.index("## SQLFluff Keyword Delta")] = (
            run_lines
        )

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"grammar oracle report written to {out}")
    return status


def main() -> int:
    args = parse_args()
    grammars_v4 = source(
        "grammars-v4 Snowflake",
        args.grammars_v4,
        GRAMMARS_V4_MARKER,
        args.grammars_v4_revision,
        GRAMMARS_V4_URL,
        "MIT (Snowflake grammar header)",
    )
    spark = source(
        "Apache Spark SQL",
        args.spark,
        SPARK_MARKER,
        args.spark_revision,
        SPARK_URL,
        "Apache-2.0",
    )
    sqlfluff = source(
        "sqlfluff",
        args.sqlfluff,
        SQLFLUFF_MARKER,
        args.sqlfluff_revision,
        SQLFLUFF_URL,
        "MIT",
    )
    out = Path(args.out)
    if not out.is_absolute():
        out = (ROOT / out).resolve()
    return write_report(
        out,
        grammars_v4,
        spark,
        sqlfluff,
        args.run_corpora,
        args.corpus_limit,
    )


if __name__ == "__main__":
    raise SystemExit(main())
