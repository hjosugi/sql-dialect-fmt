from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "grammar-oracle-report.py"
SPEC = importlib.util.spec_from_file_location("grammar_oracle_report", SCRIPT)
assert SPEC and SPEC.loader
oracle = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = oracle
SPEC.loader.exec_module(oracle)


class GrammarOracleReportTests(unittest.TestCase):
    def test_extracts_antlr_rules_without_comment_noise(self) -> None:
        grammar = """
        // fakeRule:
        parser grammar Demo;
        firstRule
            : A
            ;
        secondRule[int n] returns [int value]
            : B
            ;
        /* hiddenRule: C; */
        """
        self.assertEqual(
            oracle.extract_antlr_rules(grammar), ["firstRule", "secondRule"]
        )

    def test_extracts_string_and_list_keyword_assignments(self) -> None:
        assignments = oracle.extract_python_word_assignments(
            'reserved = """SELECT\\nFROM\\n"""\nunreserved = ["QUALIFY", "VALUE"]\n'
        )
        self.assertEqual(assignments["reserved"], {"SELECT", "FROM"})
        self.assertEqual(assignments["unreserved"], {"QUALIFY", "VALUE"})

    def test_extracts_only_segment_classes(self) -> None:
        segments = oracle.extract_python_segments(
            "class SelectStatementSegment: pass\nclass Helper: pass\n"
            "class MatchRecognizeClauseSegment(Base): pass\n"
        )
        self.assertEqual(
            segments, ["MatchRecognizeClauseSegment", "SelectStatementSegment"]
        )

    def test_locates_single_archive_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive_root = root / "project-deadbeef"
            marker = Path("nested/Marker.txt")
            (archive_root / marker).parent.mkdir(parents=True)
            (archive_root / marker).write_text("ok", encoding="utf-8")
            self.assertEqual(oracle.locate_root(str(root), marker), archive_root)

    def test_sqlfluff_reserved_and_unreserved_sets_stay_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dialects = Path(directory) / "src/sqlfluff/dialects"
            dialects.mkdir(parents=True)
            (dialects / "dialect_snowflake_keywords.py").write_text(
                'snowflake_reserved_keywords = "SELECT FROM"\n'
                'snowflake_unreserved_keywords = "QUALIFY VALUE"\n',
                encoding="utf-8",
            )
            (dialects / "dialect_databricks_keywords.py").write_text(
                'RESERVED_KEYWORDS = ["JOIN"]\nUNRESERVED_KEYWORDS = ["ZORDER"]\n',
                encoding="utf-8",
            )
            (dialects / "dialect_sparksql_keywords.py").write_text(
                'RESERVED_KEYWORDS = ["SELECT"]\nUNRESERVED_KEYWORDS = ["CACHE"]\n',
                encoding="utf-8",
            )
            (dialects / "dialect_snowflake.py").write_text(
                "class QualifyClauseSegment: pass\n", encoding="utf-8"
            )
            (dialects / "dialect_databricks.py").write_text(
                "class OptimizeTableStatementSegment: pass\n", encoding="utf-8"
            )

            inventory = oracle.inventory_sqlfluff(Path(directory))
            self.assertEqual(
                inventory["snowflake_reserved"], {"SELECT", "FROM"}
            )
            self.assertEqual(
                inventory["snowflake_unreserved"], {"QUALIFY", "VALUE"}
            )
            self.assertEqual(inventory["databricks_reserved"], {"JOIN"})
            self.assertEqual(
                inventory["databricks_unreserved"],
                {"SELECT", "CACHE", "ZORDER"},
            )

    def test_name_matching_is_explicitly_heuristic(self) -> None:
        matches = oracle.match_inventory(
            ["CreateTableStatementSegment", "CompletelyUnknownSegment"],
            {"create_table_stmt", "select_stmt"},
        )
        by_name = {match.upstream: match for match in matches}
        self.assertEqual(
            by_name["CreateTableStatementSegment"].status, "name match"
        )
        self.assertEqual(
            by_name["CreateTableStatementSegment"].local, "create_table_stmt"
        )
        self.assertEqual(by_name["CompletelyUnknownSegment"].status, "unmatched")

    def test_counts_generator_hazards_in_antlr_grammars(self) -> None:
        lexer = """
        lexer grammar DemoLexer;
        @members {
            public boolean isHint() { return false; }
        }
        SELECT: 'SELECT';
        LBRACE: '{';
        OPEN_COMMENT: '/*' {!isHint()}? .*? '*/' -> channel(HIDDEN);
        WS: [ \\t{}]+ -> skip;
        DOLLAR: '$$' -> pushMode(DOLLAR_MODE);
        fragment DIGIT: [0-9];
        mode DOLLAR_MODE;
        BODY: ~'$'+ { setText(getText().trim()); };
        """
        parser = """
        parser grammar DemoParser;
        options { tokenVocab = DemoLexer; }
        // ignored: {commentAction();}
        query
            : {!legacy_mode}? SELECT (identifier | nonReserved)
            | SELECT
            ;
        nonReserved
            : SELECT | (LBRACE | BODY) | WS
            ;
        identifier[boolean strict]
            : {strict}? BODY
            ;
        """
        hazards = oracle.generator_hazards(lexer, parser)
        self.assertEqual(hazards.parser_rules, 3)
        self.assertEqual(hazards.lexer_rules, 7)
        self.assertEqual(hazards.semantic_predicates, 3)
        self.assertEqual(hazards.inline_actions, 1)
        self.assertEqual(hazards.named_actions, ("members",))
        self.assertEqual(hazards.lexer_modes, 1)
        self.assertEqual(hazards.trivia_off_tree_rules, 2)
        self.assertEqual(
            hazards.predicate_references, ("isHint", "legacy_mode", "strict")
        )
        self.assertEqual(hazards.keyword_fallback_rules, (("nonReserved", 3),))

    def test_action_free_grammar_has_no_code_hazards(self) -> None:
        hazards = oracle.generator_hazards(
            "lexer grammar L;\nA: 'a';\nWS: [ ]+ -> channel(HIDDEN);\n",
            "parser grammar P;\nstart: A+ EOF;\nkeyword: A | A A;\n",
        )
        self.assertEqual(hazards.semantic_predicates, 0)
        self.assertEqual(hazards.inline_actions, 0)
        self.assertEqual(hazards.named_actions, ())
        self.assertEqual(hazards.trivia_off_tree_rules, 1)
        self.assertEqual(hazards.keyword_fallback_rules, (("keyword", 2),))

    def test_hazard_section_renders_one_row_per_grammar(self) -> None:
        hazards = oracle.generator_hazards(
            "lexer grammar L;\nA: 'a' {isOk()}?;\n", "parser grammar P;\nstart: A;\n"
        )
        lines = oracle.hazards_section([("Demo", hazards)])
        self.assertIn("## Generator Hazards", lines)
        self.assertIn("| Demo | 1 | 1 | 1 | 0 | — | 0 | 0 | — |", lines)
        self.assertIn("- Demo predicates reference: `isOk`", lines)


if __name__ == "__main__":
    unittest.main()
