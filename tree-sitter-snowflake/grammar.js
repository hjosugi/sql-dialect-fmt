const KEYWORDS = [
  'select',
  'from',
  'where',
  'group',
  'by',
  'having',
  'order',
  'limit',
  'offset',
  'fetch',
  'top',
  'as',
  'and',
  'or',
  'not',
  'null',
  'is',
  'in',
  'like',
  'ilike',
  'rlike',
  'regexp',
  'between',
  'case',
  'when',
  'then',
  'else',
  'end',
  'join',
  'inner',
  'left',
  'right',
  'full',
  'outer',
  'cross',
  'lateral',
  'natural',
  'on',
  'using',
  'with',
  'recursive',
  'union',
  'all',
  'any',
  'except',
  'intersect',
  'minus',
  'distinct',
  'qualify',
  'over',
  'partition',
  'window',
  'rows',
  'range',
  'unbounded',
  'preceding',
  'following',
  'current',
  'row',
  'asc',
  'desc',
  'nulls',
  'first',
  'last',
  'true',
  'false',
  'cast',
  'try_cast',
  'exists',
  'values',
  'pivot',
  'unpivot',
  'sample',
  'tablesample',
  'within',
  'create',
  'alter',
  'replace',
  'drop',
  'truncate',
  'undrop',
  'use',
  'if',
  'table',
  'view',
  'temporary',
  'temp',
  'transient',
  'volatile',
  'secure',
  'insert',
  'into',
  'update',
  'delete',
  'merge',
  'matched',
  'set',
  'overwrite',
  'flatten',
  'connect',
  'start',
  'prior',
  'language',
  'javascript',
  'python',
  'java',
  'scala',
  'sql',
  'begin',
  'declare',
  'cursor',
  'let',
  'for',
  'loop',
  'do',
  'while',
  'repeat',
  'until',
  'elseif',
  'exception',
  'resultset',
  'return',
  'call',
  'procedure',
  'function',
  'returns',
  'handler',
  'packages',
  'imports',
  'runtime_version',
  'execute',
  'immediate',
  'commit',
  'rollback',
  'owner',
  'caller',
  'strict',
  'called',
  'input',
  'output',
  'out',
  'copy',
  'grants',
  'stage',
  'file_format',
  'warehouse',
  'database',
  'schema',
  'stream',
  'task',
  'schedule',
  'after',
  'dynamic',
  'clone',
  'time',
  'travel',
  'at',
  'before',
  'changes',
  'match_recognize',
  'asof',
  'show',
  'describe',
  'grant',
  'revoke',
];

const TYPES = [
  'array',
  'bigint',
  'binary',
  'boolean',
  'char',
  'date',
  'datetime',
  'dec',
  'decimal',
  'double',
  'float',
  'geography',
  'geometry',
  'int',
  'integer',
  'map',
  'number',
  'numeric',
  'object',
  'real',
  'string',
  'text',
  'time',
  'timestamp',
  'timestamp_ltz',
  'timestamp_ntz',
  'timestamp_tz',
  'variant',
  'varchar',
  'vector',
];

const BODY_DELIMITER_RULES = [
  dollarQuotedBody,
];

// Coarse statement families, keyed by the keyword(s) that can start them. A top-level statement
// beginning with one of these words becomes the matching `<kind>_statement` node; every other
// statement stays a plain `statement`. The body of every family is the same tolerant token run,
// so unknown or newer Snowflake syntax keeps parsing \u2014 only the node name changes.
const STATEMENT_KINDS = [
  ['select_statement', ['select', 'with']],
  ['insert_statement', ['insert']],
  ['update_statement', ['update']],
  ['delete_statement', ['delete']],
  ['merge_statement', ['merge']],
  ['create_statement', ['create']],
  ['drop_statement', ['drop']],
  ['alter_statement', ['alter']],
  ['grant_statement', ['grant']],
  ['revoke_statement', ['revoke']],
  ['copy_statement', ['copy']],
  ['use_statement', ['use']],
  ['set_statement', ['set']],
  ['show_statement', ['show']],
  ['describe_statement', ['describe', 'desc']],
];

// Statement-leading keywords lex as dedicated (hidden) tokens so the parser can pick a statement
// family from the first word. They are always aliased back to `keyword` in the tree, keeping the
// token stream \u2014 and every highlight/injection query over it \u2014 unchanged. KEYWORDS above remains
// the single source of truth that the Rust keyword-sync tests check.
const STATEMENT_LEADING_KEYWORDS = [...new Set(STATEMENT_KINDS.flatMap(([, words]) => words))];

const GENERAL_KEYWORDS = KEYWORDS.filter(word => !STATEMENT_LEADING_KEYWORDS.includes(word));

// The most deeply nested structural `{ ... }` a `${ ... }` placeholder body is balanced through.
// Token regexes cannot recurse, so the nesting is expanded to this fixed depth -- far beyond what
// realistic templated SQL uses. Quoted strings are matched at every level, so a `}` inside a string
// never closes the placeholder regardless of depth. Declared before `grammar(...)` (unlike the
// hoisted helper functions below) because the rule closures read it while the grammar is built.
const PLACEHOLDER_MAX_DEPTH = 4;

module.exports = grammar({
  name: 'snowflake',

  extras: $ => [
    /[\s\uFEFF\u2060]+/,
    $.comment,
  ],

  word: $ => $.identifier,

  rules: {
    source_file: $ => repeat($._statement),

    _statement: $ => choice(
      ...STATEMENT_KINDS.map(([kind]) => $[kind]),
      $.statement,
    ),

    // A top-level statement: a tolerant run of tokens up to (and including) its `;` terminator. The
    // last statement in a script may omit the terminator, and a bare `;` is an empty statement.
    // Inside the run we opportunistically group balanced parentheses and immediate call syntax into
    // expression nodes. The grammar remains deliberately permissive: the rowan CST parser is still
    // the formatter source of truth.
    //
    // `statement` is the lenient fallback for anything that does not start with a statement-leading
    // keyword (scripting blocks, CALL, TRUNCATE, EXECUTE IMMEDIATE, ...). Its first token excludes
    // the leading keywords so that those always open their dedicated `<kind>_statement` node;
    // mid-statement occurrences (subqueries, `UPDATE ... SET`, `GRANT SELECT`, ...) stay inside the
    // current statement via the same right-associative "keep consuming" rule used before.
    statement: $ => choice(
      prec.right(seq($._non_leading_item, repeat($._statement_item), optional(';'))),
      ';',
    ),

    ...Object.fromEntries(STATEMENT_KINDS.map(([kind, words]) => [
      kind,
      $ => prec.right(seq(
        choice(...words.map(value => alias($[leadingTokenName(value)], $.keyword))),
        repeat($._statement_item),
        optional(';'),
      )),
    ])),

    _statement_item: $ => choice(
      $.expression,
      $._token,
    ),

    _non_leading_item: $ => choice(
      $.expression,
      $._non_leading_token,
    ),

    expression: $ => choice(
      $.call_expression,
      $.parenthesized_expression,
    ),

    call_expression: $ => prec(2, seq(
      field('function', $._callee),
      field('arguments', $.argument_list),
    )),

    argument_list: $ => seq(
      token.immediate('('),
      repeat(choice($.expression, $._token)),
      ')',
    ),

    parenthesized_expression: $ => seq(
      '(',
      repeat(choice($.expression, $._token)),
      ')',
    ),

    _callee: $ => choice(
      $.identifier,
      $.quoted_identifier,
      $.keyword,
      $.type,
    ),

    _token: $ => choice(
      $._non_leading_token,
      $._leading_keyword,
    ),

    _non_leading_token: $ => choice(
      $.stage_reference,
      $.keyword,
      $.type,
      $.dollar_string,
      $.string,
      $.quoted_identifier,
      $.number,
      $.placeholder,
      $.variable,
      $.identifier,
      $.operator,
      $.punctuation,
    ),

    // Statement-leading keywords appearing mid-statement (or inside parentheses) surface as plain
    // `keyword` nodes, exactly as before the statement families were introduced.
    _leading_keyword: $ => choice(
      ...STATEMENT_LEADING_KEYWORDS.map(value => alias($[leadingTokenName(value)], $.keyword)),
    ),

    ...Object.fromEntries(STATEMENT_LEADING_KEYWORDS.map(value => [
      leadingTokenName(value),
      _ => token(prec(2, word(value))),
    ])),

    comment: _ => token(prec(10, choice(
      seq('--', /[^\r\n]*/),
      seq('//', /[^\r\n]*/),
      seq('/*', repeat(choice(/[^*]/, seq('*', /[^/]/))), repeat('*'), '/'),
    ))),

    stage_reference: _ => token(prec(4, seq(
      '@',
      repeat1(choice(
        /[A-Za-z0-9_$%~./:=+-]+/,
        seq('"', repeat(choice(/[^"]/, '""')), '"'),
      )),
    ))),

    keyword: _ => token(prec(2, choice(...GENERAL_KEYWORDS.map(word)))),

    type: _ => token(prec(2, choice(...TYPES.map(word)))),

    identifier: _ => /[A-Za-z_][A-Za-z0-9_$]*/,

    quoted_identifier: _ => token(seq(
      '"',
      repeat(choice(/[^"]/, '""')),
      '"',
    )),

    string: _ => token(seq(
      "'",
      repeat(choice(/[^'\\]/, /\\./, "''")),
      "'",
    )),

    dollar_string: _ => token(choice(...BODY_DELIMITER_RULES.map(rule => rule()))),

    number: _ => token(choice(
      /\d+\.\d*([eE][+-]?\d+)?/,
      /\.\d+([eE][+-]?\d+)?/,
      /\d+([eE][+-]?\d+)?/,
    )),

    variable: _ => token(seq('$', choice(/[0-9]+/, /[A-Za-z_][A-Za-z0-9_$]*/))),

    // A `${ ... }` template-substitution placeholder: a JS template literal interpolation
    // (`${cfg.table}`), or Databricks / Spark / dbt variable substitution (`${env:VAR}`). SQL is
    // routinely embedded in a host language, so -- like `$$...$$` -- the placeholder is kept as one
    // coarse token, letting the surrounding statement keep parsing and be highlighted as a unit.
    // See `placeholderBody` for how nested braces and quoted `}` are balanced.
    placeholder: _ => token(seq('${', repeat(placeholderBody(PLACEHOLDER_MAX_DEPTH)), '}')),

    operator: _ => token(prec(3, choice(
      '->>',
      '|>',
      '::',
      ':=',
      '=>',
      '->',
      '||',
      '<=>',
      '<=',
      '>=',
      '<>',
      '!=',
      '=',
      '<',
      '>',
      '+',
      '-',
      '*',
      '/',
      '%',
      ':',
      '|',
      '&',
      '^',
      '~',
      '@',
      '?',
      '!',
    ))),

    punctuation: _ => token(choice(
      '(',
      ')',
      '[',
      ']',
      '{',
      '}',
      ',',
      '.',
    )),
  },
});

function word(value) {
  const escaped = value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(escaped.replace(/[A-Za-z]/g, char => `[${char.toLowerCase()}${char.toUpperCase()}]`));
}

function leadingTokenName(value) {
  return `_${value}_leading_keyword`;
}

function dollarQuotedBody() {
  return seq(
    '$$',
    repeat(choice(/[^$]/, seq('$', /[^$]/))),
    '$$',
  );
}

// A single-, double-, or back-tick-quoted string inside a placeholder body, consumed opaquely so
// that a `}` -- or an inner `${...}` inside a template literal -- cannot terminate the placeholder.
function placeholderString() {
  return choice(
    seq("'", repeat(choice(/[^'\\]/, /\\./)), "'"),
    seq('"', repeat(choice(/[^"\\]/, /\\./)), '"'),
    seq('`', repeat(choice(/[^`\\]/, /\\./)), '`'),
  );
}

// One element of a placeholder body: a run of ordinary characters, a quoted string, or -- until the
// depth budget is spent -- a balanced nested `{ ... }` group whose own body recurses one level down.
function placeholderBody(depth) {
  const parts = [/[^{}'"`]+/, placeholderString()];
  if (depth > 0) {
    parts.push(seq('{', repeat(placeholderBody(depth - 1)), '}'));
  }
  return choice(...parts);
}
