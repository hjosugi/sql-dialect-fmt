; A Snowflake UDF/procedure `$$ … $$` body is written in the language named by the statement's
; LANGUAGE clause (JavaScript, Python, Java, Scala, or SQL). Inject that language so the body is
; highlighted by its own grammar. `#offset!` trims the two-character `$$` delimiters so only the
; body text is handed to the injected language.
(create_statement
  (language_clause
    name: (_) @injection.language)
  (dollar_string) @injection.content
  (#offset! @injection.content 0 2 0 -2))
