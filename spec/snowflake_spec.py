#!/usr/bin/env python3
"""sql-dialect-fmt Snowflake spec tracker.

A tiny, dependency-free (stdlib `sqlite3`) store for the Snowflake SQL surface and how it
changes over time. It lives OUTSIDE the Cargo workspace (this `spec/` dir is not a `crates/*`
member), so it never affects `cargo build`. Responding to spec drift is manual — this tool just
records what changed and when, so a human can update the parser/ROADMAP deliberately.

Usage:
    python3 spec/snowflake_spec.py init
    python3 spec/snowflake_spec.py import spec/seed/features.json --note "2026-06 refresh"
    python3 spec/snowflake_spec.py coverage
    python3 spec/snowflake_spec.py changes [--limit N]
    python3 spec/snowflake_spec.py snapshot --note "..."
"""
import argparse
import json
import os
import sqlite3
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
DB = os.path.join(HERE, "snowflake_spec.db")
TRACKED = ["category", "syntax", "status", "coverage", "source", "notes"]


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def connect() -> sqlite3.Connection:
    return sqlite3.connect(DB)


def cmd_init(_args) -> None:
    con = connect()
    con.executescript(
        """
        CREATE TABLE IF NOT EXISTS feature (
            name TEXT PRIMARY KEY, category TEXT, syntax TEXT, status TEXT,
            coverage TEXT, source TEXT, notes TEXT,
            first_seen TEXT, last_seen TEXT, last_changed TEXT
        );
        CREATE TABLE IF NOT EXISTS snapshot (
            id INTEGER PRIMARY KEY AUTOINCREMENT, taken_at TEXT, note TEXT,
            n_features INTEGER, n_added INTEGER, n_changed INTEGER
        );
        CREATE TABLE IF NOT EXISTS change (
            id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER,
            name TEXT, field TEXT, old TEXT, new TEXT, at TEXT
        );
        """
    )
    con.commit()
    con.close()
    print(f"initialized {DB}")


def cmd_import(args) -> None:
    with open(args.json, encoding="utf-8") as f:
        data = json.load(f)
    feats = data["features"] if isinstance(data, dict) else data
    con = connect()
    cur = con.cursor()
    ts = now()
    cur.execute(
        "INSERT INTO snapshot (taken_at, note, n_features, n_added, n_changed) VALUES (?,?,?,?,?)",
        (ts, args.note, len(feats), 0, 0),
    )
    snap = cur.lastrowid
    n_added = n_changed = 0
    for ft in feats:
        name = ft["name"]
        row = cur.execute(
            "SELECT %s FROM feature WHERE name=?" % ",".join(TRACKED), (name,)
        ).fetchone()
        if row is None:
            cur.execute(
                "INSERT INTO feature (name,category,syntax,status,coverage,source,notes,"
                "first_seen,last_seen,last_changed) VALUES (?,?,?,?,?,?,?,?,?,?)",
                (
                    name, ft.get("category", ""), ft.get("syntax", ""), ft.get("status", ""),
                    ft.get("coverage", ""), ft.get("source", ""), ft.get("notes", ""), ts, ts, ts,
                ),
            )
            cur.execute(
                "INSERT INTO change (snapshot_id,name,field,old,new,at) VALUES (?,?,?,?,?,?)",
                (snap, name, "(added)", "", ft.get("status", ""), ts),
            )
            n_added += 1
        else:
            changed = False
            for i, field in enumerate(TRACKED):
                old = row[i] or ""
                new = ft.get(field, "") or ""
                if old != new:
                    cur.execute(
                        "INSERT INTO change (snapshot_id,name,field,old,new,at) VALUES (?,?,?,?,?,?)",
                        (snap, name, field, old, new, ts),
                    )
                    changed = True
            cur.execute(
                "UPDATE feature SET category=?,syntax=?,status=?,coverage=?,source=?,notes=?,last_seen=? "
                "WHERE name=?",
                (
                    ft.get("category", ""), ft.get("syntax", ""), ft.get("status", ""),
                    ft.get("coverage", ""), ft.get("source", ""), ft.get("notes", ""), ts, name,
                ),
            )
            if changed:
                cur.execute("UPDATE feature SET last_changed=? WHERE name=?", (ts, name))
                n_changed += 1
    cur.execute(
        "UPDATE snapshot SET n_added=?, n_changed=? WHERE id=?", (n_added, n_changed, snap)
    )
    con.commit()
    con.close()
    print(f"snapshot #{snap}: {len(feats)} features, {n_added} added, {n_changed} changed")


def cmd_coverage(_args) -> None:
    con = connect()
    cur = con.cursor()
    total = cur.execute("SELECT COUNT(*) FROM feature").fetchone()[0]
    print(f"features: {total}")
    print("by coverage:")
    for cov, n in cur.execute(
        "SELECT COALESCE(NULLIF(coverage,''),'(unset)') c, COUNT(*) FROM feature GROUP BY c ORDER BY 2 DESC"
    ):
        print(f"  {cov:10} {n}")
    print("by category (parsed/total):")
    for cat, tot, parsed in cur.execute(
        "SELECT category, COUNT(*), SUM(CASE WHEN coverage='parse' THEN 1 ELSE 0 END) "
        "FROM feature GROUP BY category ORDER BY category"
    ):
        print(f"  {cat:14} {parsed or 0}/{tot}")
    con.close()


def cmd_changes(args) -> None:
    con = connect()
    cur = con.cursor()
    rows = cur.execute(
        "SELECT at, name, field, old, new FROM change ORDER BY id DESC LIMIT ?", (args.limit,)
    ).fetchall()
    for at, name, field, old, new in rows:
        if field == "(added)":
            print(f"{at}  + {name} ({new})")
        else:
            print(f"{at}  ~ {name}.{field}: {old!r} -> {new!r}")
    con.close()


def cmd_snapshot(args) -> None:
    con = connect()
    cur = con.cursor()
    ts = now()
    n = cur.execute("SELECT COUNT(*) FROM feature").fetchone()[0]
    cur.execute(
        "INSERT INTO snapshot (taken_at,note,n_features,n_added,n_changed) VALUES (?,?,?,0,0)",
        (ts, args.note, n),
    )
    con.commit()
    con.close()
    print(f"snapshot recorded at {ts} ({n} features)")


def main() -> None:
    ap = argparse.ArgumentParser(description="sql-dialect-fmt Snowflake spec tracker")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("init")
    p = sub.add_parser("import")
    p.add_argument("json")
    p.add_argument("--note", default="")
    sub.add_parser("coverage")
    p = sub.add_parser("changes")
    p.add_argument("--limit", type=int, default=50)
    p = sub.add_parser("snapshot")
    p.add_argument("--note", default="")
    args = ap.parse_args()
    {
        "init": cmd_init,
        "import": cmd_import,
        "coverage": cmd_coverage,
        "changes": cmd_changes,
        "snapshot": cmd_snapshot,
    }[args.cmd](args)


if __name__ == "__main__":
    main()
