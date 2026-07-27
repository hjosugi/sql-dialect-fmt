#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/run-external-corpus.sh --sample [--dialect snowflake|databricks] [--limit N]
  scripts/run-external-corpus.sh --path PATH [--dialect snowflake|databricks] [--limit N]
  scripts/run-external-corpus.sh --url URL [--dialect snowflake|databricks] [--limit N]

Runs the formatter invariant harness over a committed sample corpus, a local
corpus path, or a downloaded .tar.gz/.tgz/.tar/.zip archive.
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE=""
CORPUS_VALUE=""
LIMIT=""
DIALECT="snowflake"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sample)
      MODE="sample"
      shift
      ;;
    --path)
      MODE="path"
      CORPUS_VALUE="${2:?missing value for --path}"
      shift 2
      ;;
    --url)
      MODE="url"
      CORPUS_VALUE="${2:?missing value for --url}"
      shift 2
      ;;
    --limit)
      LIMIT="${2:?missing value for --limit}"
      shift 2
      ;;
    --dialect)
      DIALECT="${2:?missing value for --dialect}"
      case "$DIALECT" in
        snowflake|databricks) ;;
        *)
          echo "unsupported dialect: $DIALECT" >&2
          exit 2
          ;;
      esac
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$MODE" ]]; then
  usage >&2
  exit 2
fi

TMP_DIR=""
cleanup() {
  if [[ -n "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT

case "$MODE" in
  sample)
    CORPUS_ROOT="$ROOT_DIR/crates/sql-dialect-fmt-formatter/tests/corpus_sample"
    ;;
  path)
    CORPUS_ROOT="$CORPUS_VALUE"
    ;;
  url)
    TMP_DIR="$(mktemp -d)"
    archive="$TMP_DIR/corpus"
    curl -fsSL "$CORPUS_VALUE" -o "$archive"
    mkdir -p "$TMP_DIR/unpacked"
    case "$CORPUS_VALUE" in
      *.tar.gz|*.tgz)
        tar -xzf "$archive" -C "$TMP_DIR/unpacked"
        ;;
      *.tar)
        tar -xf "$archive" -C "$TMP_DIR/unpacked"
        ;;
      *.zip)
        unzip -q "$archive" -d "$TMP_DIR/unpacked"
        ;;
      *)
        echo "unsupported corpus archive extension: $CORPUS_VALUE" >&2
        exit 2
        ;;
    esac
    CORPUS_ROOT="$TMP_DIR/unpacked"
    ;;
esac

export SQL_DIALECT_FMT_EXTERNAL_CORPUS="$CORPUS_ROOT"
export SQL_DIALECT_FMT_EXTERNAL_CORPUS_DIALECT="$DIALECT"
if [[ -n "$LIMIT" ]]; then
  export SQL_DIALECT_FMT_EXTERNAL_CORPUS_LIMIT="$LIMIT"
fi

cd "$ROOT_DIR"
cargo test -p sql-dialect-fmt-formatter --test external_corpus -- --ignored --nocapture
