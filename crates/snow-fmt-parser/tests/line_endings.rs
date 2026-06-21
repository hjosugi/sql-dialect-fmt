//! Parser line-ending resilience.

use snow_fmt_test_support::parser::{
    assert_parse_clean as clean, assert_parse_roundtrip as roundtrip,
};

#[test]
fn clean_selects_with_linux_windows_and_old_mac_newlines() {
    for newline in ["\n", "\r\n", "\r"] {
        let sql = format!(
            "SELECT a,{newline}       b AS name{newline}FROM table_name{newline}WHERE a IS NOT NULL{newline}ORDER BY b DESC{newline}LIMIT 10"
        );
        clean(&sql);
    }
}

#[test]
fn mixed_newline_file_roundtrips_with_comments() {
    let sql = "/* header\r\ncomment */\rSELECT a FROM t WHERE a = 1;\n-- next\r\nSELECT b FROM u WHERE b BETWEEN 1 AND 2;\r";
    clean(sql);
}

#[test]
fn recovery_preserves_newlines_in_broken_input() {
    for sql in [
        "SELECT\r\nFROM\r\nWHERE",
        "WITH c AS\rSELECT 1\nSELECT * FROM c",
        "SELECT a FROM t WHERE a IN (\r\n  1,\r  2,\n",
    ] {
        roundtrip(sql);
    }
}

#[test]
fn long_crlf_query_is_clean_and_lossless() {
    let mut sql = String::from("SELECT\r\n");
    for i in 0..256 {
        if i > 0 {
            sql.push_str(",\r\n");
        }
        sql.push_str("    c");
        sql.push_str(&i.to_string());
        sql.push_str(" AS alias_");
        sql.push_str(&i.to_string());
    }
    sql.push_str("\r\nFROM wide_table\r\nWHERE c0 IS NOT NULL\r\nORDER BY c0\r\nLIMIT 50");

    clean(&sql);
}
