; Embedded-language injections for Snowflake dollar-quoted bodies.
;
; The grammar keeps `$$ ... $$` as a single `dollar_string` token for robustness.
; These queries add context from the surrounding statement where Snowflake exposes
; it: routine bodies declare `LANGUAGE <name> ... AS $$...$$`, while dynamic SQL
; commonly appears as `EXECUTE IMMEDIATE $$...$$`.

((statement
  (keyword) @_language
  .
  (keyword) @_javascript
  (_)*
  (keyword) @_as
  .
  (dollar_string) @injection.content)
  (#match? @_language "^[Ll][Aa][Nn][Gg][Uu][Aa][Gg][Ee]$")
  (#match? @_javascript "^[Jj][Aa][Vv][Aa][Ss][Cc][Rr][Ii][Pp][Tt]$")
  (#match? @_as "^[Aa][Ss]$")
  (#set! injection.language "javascript"))

((statement
  (keyword) @_language
  .
  (keyword) @_python
  (_)*
  (keyword) @_as
  .
  (dollar_string) @injection.content)
  (#match? @_language "^[Ll][Aa][Nn][Gg][Uu][Aa][Gg][Ee]$")
  (#match? @_python "^[Pp][Yy][Tt][Hh][Oo][Nn]$")
  (#match? @_as "^[Aa][Ss]$")
  (#set! injection.language "python"))

((statement
  (keyword) @_language
  .
  (keyword) @_java
  (_)*
  (keyword) @_as
  .
  (dollar_string) @injection.content)
  (#match? @_language "^[Ll][Aa][Nn][Gg][Uu][Aa][Gg][Ee]$")
  (#match? @_java "^[Jj][Aa][Vv][Aa]$")
  (#match? @_as "^[Aa][Ss]$")
  (#set! injection.language "java"))

((statement
  (keyword) @_language
  .
  (keyword) @_scala
  (_)*
  (keyword) @_as
  .
  (dollar_string) @injection.content)
  (#match? @_language "^[Ll][Aa][Nn][Gg][Uu][Aa][Gg][Ee]$")
  (#match? @_scala "^[Ss][Cc][Aa][Ll][Aa]$")
  (#match? @_as "^[Aa][Ss]$")
  (#set! injection.language "scala"))

((statement
  (keyword) @_language
  .
  (keyword) @_sql
  (_)*
  (keyword) @_as
  .
  (dollar_string) @injection.content)
  (#match? @_language "^[Ll][Aa][Nn][Gg][Uu][Aa][Gg][Ee]$")
  (#match? @_sql "^[Ss][Qq][Ll]$")
  (#match? @_as "^[Aa][Ss]$")
  (#set! injection.language "sql"))

((statement
  (keyword) @_execute
  .
  (keyword) @_immediate
  .
  (dollar_string) @injection.content)
  (#match? @_execute "^[Ee][Xx][Ee][Cc][Uu][Tt][Ee]$")
  (#match? @_immediate "^[Ii][Mm][Mm][Ee][Dd][Ii][Aa][Tt][Ee]$")
  (#set! injection.language "sql"))
