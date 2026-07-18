<!-- i18n: language-switcher -->
[English](prior-art.md) | [日本語](prior-art.ja.md)

# sql-dialect-fmt: 先行研究と抽出された教訓

## 概要

最高水準の Snowflake SQL フォーマッタ＋ハイライターは未開拓のニッチです。既存の SQL ツールは、*{コメントを損失なく保持する CST}* × *{深い Snowflake 文法}* × *{Prettierスタイルの Doc-IR シングルパスプリティプリンタ}* × *{Rust の高速性}* のいずれかを必ず満たしていません。勝利の設計図は、Biome や Ruff が JS/Python で証明したものです：手書きのエラー耐性を持つ再帰下降パーサーがフラットな**イベントストリーム**を出力し、それが**rowan**の損失なしグリーン/レッド CST（rust-analyzer方式）を構築します；薄い型付き AST レイヤー；汎用的な**Doc-IR フォーマッタエンジン**（`FormatElement`、`group`/`line`/`indent`/`best_fitting`）は Prettier → `rome_formatter` → `biome_formatter`/`ruff_formatter` の系譜；各ノードごとの**先頭/末尾/ぶら下がり**コメントの付与；末尾コメントは `line_suffix` で処理；埋め込み `$$ … $$` 本体は `biome_js_formatter` に渡し、`markAsRoot`/`dedentToRoot` 方式で再インデント；ハイライトは段階的に**LSP セマンティックトークン → TextMate → tree-sitter**；スナップショット（`insta`）＋冪等性＋ Ruff スタイルの類似度スコアコーパスハーネスで適合性を証明。sql-dialect-fmt のレキサーは完成済みなので、直近の作業はパーサーイベント＋CST＋Docエンジンです。

## プロジェクト比較

| プロジェクト | 言語 | 借用 | 回避 | ソース |
|---|---|---|---|---|
| **Biome** (`biome_formatter`, `biome_js_formatter`, `biome_rowan`) | Rust | `FormatElement` IR；`Format`/`FormatRule`/`FormatNodeRule` トレイト；`write!`/`format!` マクロ；`place_comment` 先頭/末尾/ぶら下がり；forked-rowan CST；埋め込み CSS-in-JS スニペットパス | IR を JS 固有に過度に合わせない；大規模なマクロ | [biome](https://github.com/biomejs/biome), [formatter impl](https://deepwiki.com/biomejs/biome/6.2-formatter-implementation) |
| **rust-analyzer / rowan** | Rust | グリーン/レッドツリー；`GreenNodeBuilder`＋`checkpoint`；パーサー `Event` ストリーム；`Marker`/`CompletedMarker`（`complete`/`abandon`/`precede`）；`TokenSet` リカバリ；ungrammar コード生成 | 文法が安定するまで ungrammar コード生成は過剰 | [rust-analyzer](https://github.com/rust-lang/rust-analyzer), [rowan](https://github.com/rust-analyzer/rowan) |
| **tree-sitter** | C/Rust | `.scm` ハイライト/インジェクションクエリ；Neovim キャプチャ名語彙；エディタ向けインクリメンタル再解析 | *フォーマッタ* パーサーには不向き：コメントは `extras` に浮遊、弱い診断、GLR コンフリクト | [tree-sitter](https://tree-sitter.github.io/tree-sitter/) |
| **Topiary** | Rust | フォーマット決定の*チェックリスト*としてのキャプチャセット；「インジェクションは葉」アイデア | クエリベースのフォーマットは幅測定不可；トークン圧縮/意味破壊のリスク | [topiary](https://github.com/tweag/topiary) |
| **Prettier** | JS | Doc コマンドセット；コメント付与アルゴリズム（`decorateComment`）；`markAsRoot`/`dedentToRoot`；冪等性規範 | 動的/非型付け Doc；非同期埋め込みパイプライン | [commands.md](https://github.com/prettier/prettier/blob/main/commands.md) |
| **Ruff** (`ruff_formatter`, `ruff_python_formatter`) | Rust | 2クレート分割；魔法の末尾カンマ；`comments/placement.rs` ヒューリスティクス；`--stability-check`；`format-dev` 類似度スコア；プラグマコメントは幅から除外 | Black 固有のスタイルルール | [ruff](https://github.com/astral-sh/ruff), [formatter docs](https://docs.astral.sh/ruff/formatter/) |
| **dprint** | Rust | カラム対応 `Condition`/`Info` を狭いアラインメント逃げ道として利用 | デフォルトが完全命令型 IR；Wasm プラグインプラットフォーム | [dprint](https://github.com/dprint/dprint) |
| **sqlparser-rs** | Rust | `Dialect` トレイト＋`SnowflakeDialect` を文法参照として利用；`tests/sqlparser_snowflake.rs` SQL をフィクスチャとして活用（Apache-2.0） | コメントを落とすロスのある AST — フォーマッタの基盤には不可 | [datafusion-sqlparser-rs](https://github.com/apache/datafusion-sqlparser-rs) |
| **sqlfluff** | Python | `dialect_snowflake.py`（約10.7k行）＝OSSで最良の Snowflake 文法；損失なしセグメントツリー；テンプレートスライスマップ | ループ型ルールベース「fix」（非冪等、揺れる）；Python＝遅い | [sqlfluff](https://github.com/sqlfluff/sqlfluff) |
| **sql-formatter** | JS | 大文字小文字設定（キーワード/識別子/データ型/関数）；`logicalOperatorNewline`；密なオペレータアイデア | トークンストリームのみ、実ツリーなし→インラインif-fit不可（[#631](https://github.com/sql-formatter-org/sql-formatter/issues/631)）；浅い Snowflake | [sql-formatter](https://github.com/sql-formatter-org/sql-formatter) |
| **libpg_query** | C | 「実文法を埋め込む」忠実性の教訓 | Postgres専用；コメントを落とす；deparseは単一行 | [libpg_query](https://github.com/pganalyze/libpg_query) |
| **uroborosql-fmt / postgresql-cst-parser** | Rust | 上流 PostgreSQL 文法から生成された純Rust CSTパーサー；公式文法/例を使いパーサーメンテコストを圧縮；ブラウザ/VS Code/CLIを一級配布ターゲットに；LSP診断を自然なフォローオンとして | PostgreSQL専用；公式文法生成は上流構文ソースが利用可能かつ損失なし/コメント保持フォーマッタ契約が必要な場合のみ有効 | [uroborosql-fmt](https://github.com/future-architect/uroborosql-fmt), [postgresql-cst-parser](https://github.com/future-architect/postgresql-cst-parser) |
| **gofmt / zig fmt** | Go/Zig | ゼロ設定、意見先行で早期出荷；末尾カンマをユーザーの「操舵」信号に | スタイル設定を追加しない | [gofmt](https://wordaligned.org/articles/gofmt-knows-best), [zig fmt](https://matklad.github.io/2026/05/08/steering-zig-fmt.html) |

## 領域別推奨事項

### 1. CST 構築 — rowan ビルダー＋パーサーイベント
**rowan（またはそのグリーン/レッド分割をコピー）を使い、パーサーとツリー構築をイベントストリームで分離する。** グリーンノードはイミュータブル、位置情報なし、重複排除（トリビアトークンも一級でバイト単位往復可能）；レッドノードは親/オフセットを遅延追加 — 損失なしフォーマッタCSTに理想的。*理由：*これは rust-analyzer の実績ある設計で、Biome もフォークしています。（[rowan green/red](https://github.com/rust-analyzer/rowan), [RA syntax book](https://rust-analyzer.github.io/book/contributing/syntax.html)）
- フラットな `#[repr(u16)] SyntaxKind` enum（トークン＋ノード種別）を定義；rowan の `Language` を `SnowLang {}` マーカーで実装；`T![,]`/`T![select]` マクロを追加。（[RA syntax_kind](https://github.com/rust-lang/rust-analyzer/blob/master/crates/parser/src/syntax_kind/generated.rs)）
- パーサーは `Vec<Event>`（`Start{kind,forward_parent}` / `Token` / `Finish` / `Error`）をプッシュ；別パスでイベントを `GreenNodeBuilder` に流し、**ここで空白/コメントを付与**して文法はクリーン、ツリーは損失なしに。（[RA event.rs](https://github.com/rust-lang/rust-analyzer/blob/master/crates/parser/src/event.rs)）
- `Marker`/`CompletedMarker` API＋`DropBomb` を使う：`m=p.start()`, `m.complete(p,KIND)`, `m.abandon(p)`（バックトラック）、`cm.precede(p)` は左結合SQL（`a.b.c`, `t1 JOIN t2 JOIN t3`, `a OR b OR c`）用。`GreenNodeBuilder::checkpoint`/`start_node_at` は後付ラップのトリック。（[RA parser.rs](https://github.com/rust-lang/rust-analyzer/blob/master/crates/parser/src/parser.rs), [GreenNodeBuilder](https://docs.rs/rowan/latest/rowan/struct.GreenNodeBuilder.html)）
- **「パースは決して失敗しない」**を原則に：`(SyntaxNode, Vec<SyntaxError>)` を返し、迷子トークンは ERROR ノードでラップ、`TokenSet` FOLLOWセット（句開始キーワード/`;`）でリカバリ、ループは燃料カウンタでガード。直接適用可能な設計は matklad の [Resilient LL Parsing](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)；SQL の多段階優先度には [Pratt parsing](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html) を使う。
- 薄い型付き AST newtype（`struct SelectClause{syntax:SyntaxNode}`、`cast`/`syntax`/`support::child`）を追加。文法が固まるまで ungrammar コード生成は後回し。（[RA AstNode](https://github.com/rust-lang/rust-analyzer/blob/master/crates/syntax/src/ast/generated/nodes.rs)）
- **公式仕様由来のパーサー/適合性レーン**を維持。`postgresql-cst-parser` のパターンが有用：上流文法や機械可読構文インベントリがあれば、CSTパーサー表面を純Rustで最大限生成し、新構文対応はほぼフォーマットルール作業に。Snowflake は PostgreSQL の `gram.y` のようなものを公開していないので、現実的な近期版はドキュメント/例をマイニングし、カバレッジフィクスチャ、キーワード差分、ステートメント骨格、パーサーギャップレポートを出力。手書きパーサーを置き換えるのは、生成パスがトリビア保持、部分入力からの回復、コンパクトさ、Wasm へのクリーンコンパイルを満たしてから。

### 2. フォーマッタ IR — biome/ruff をモデルに Doc エンジンを構築
**汎用的な `snow_formatter` エンジンクレート（`biome_formatter` に直接依存しない）を構築し、IR は `FormatElement` をコピー、Snowflake 固有知識は `snow_formatter_sql` レイヤーに集約。** 実績ある系譜は Prettier Doc → `rome_formatter` → `{biome_formatter, ruff_formatter}`；Ruff は「Biome の優れたプリンタを明示的に適用」と述べています。*理由：*幅測定型 Wadler/Prettier プリンタだけが短い `SELECT` を1行に収め、長いものは幅制限で折り返せます — トークンストリーム型（sql-formatter）や入力再整形型（Topiary, sqlfluff）は不可。（[ruff_formatter lib.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/lib.rs), [Ruff blog](https://astral.sh/blog/the-ruff-formatter), [Biome formatter](https://deepwiki.com/biomejs/biome/6.2-formatter-implementation)）
- **IR バリアント：** `Text` / `SourceCodeSlice`（ゼロコピー）、`Line(LineMode::{Soft,Hard,Empty,SoftOrSpace})`、`Tag(StartGroup/EndGroup/StartIndent/EndIndent…)`、`LineSuffix`＋`LineSuffixBoundary`、`BestFitting`（レイアウト試行、最初に収まるものを選択＝Prettier `conditionalGroup`）、`Interned`、`ExpandParent`。（[ruff format_element.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/format_element.rs), [Prettier commands.md](https://github.com/prettier/prettier/blob/main/commands.md)）
- **ビルダー：** `group`, `indent`/`dedent`, `block_indent`/`soft_block_indent`, 4種の改行, `line_suffix`, `if_group_breaks`, `best_fitting`。（[ruff builders.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/builders.rs)）
- **トレイト：** バッファベース `Format<Context>`＋`FormatRule<T,Context>`＋`FormatNodeRule<N>`（パーサークレートのAST型を孤立implなしでフォーマット）、`write!`/`format_args!`/`format!` マクロ。（[Biome traits](https://deepwiki.com/biomejs/biome/6.2-formatter-implementation)）
- dprintスタイルのカラム対応 `Condition`/`Info` は*狭い*逃げ道として（エイリアス/`ON`述語アラインメントのみ）—デフォルトにはしない。（[dprint](https://github.com/dprint/dprint)）
- **魔法の末尾カンマ**（最も効果的な機能）：ユーザーが `SELECT` リスト / `IN (...)` / `VALUES` / 引数リストに末尾カンマを付けると展開レイアウトを強制（`expand_parent` を注入）、なければ折り畳み可能。これは zig-fmt の「操舵」信号で、往復で維持されます。（[Ruff black deviations](https://docs.astral.sh/ruff/formatter/black/), [zig fmt](https://matklad.github.io/2026/05/08/steering-zig-fmt.html)）

### 3. コメント付与 — ノードごとに先頭/末尾/ぶら下がり
**コメントはノードごと（トークンごとではなく）に3バケットで付与し、末尾コメントは `line_suffix` で出力。** Biome（`place_comment` → `CommentPlacement`, `DecoratedComment`, `CommentStyle`）も Ruff（`comments/placement.rs`）もこれを採用；Ruff はノードごと付与で「組み合わせが大幅減、十分精度」と述べています。*理由：*位置が少ない＝扱いやすく冪等。（[Biome comments](https://deepwiki.com/biomejs/biome/6.2-formatter-implementation), [Ruff comments/mod.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_formatter/src/comments/mod.rs)）
- アルゴリズム（Prettier `decorateComment`/`attachComments`）：各コメントについて `enclosingNode`, `precedingNode`, `followingNode` を探索；**followingNode ⇒ 先頭**, **precedingNode ⇒ 末尾**, **どちらもなし ⇒ ぶら下がり**（括弧内の単独コメントなど）。`isOwnLineComment`/`isEndOfLineComment`（前後改行）と `breakTies` で曖昧ケースを処理。（[Prettier attach.js](https://github.com/prettier/prettier/blob/main/src/main/comments/attach.js)）
- SQL固有のオーバーライドハンドラ（Ruff の `handle_*` ヒューリスティクスのように）：`SELECT` 後、`WHEN`/`THEN` 間、JOIN の `ON` 前、括弧内など。（[Ruff placement.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_formatter/src/comments/placement.rs)）
- **ディレクティブコメントは幅から除外**（Ruff は `# noqa`/`# type:` を除外）：長い末尾 `-- noqa`/`-- sql-dialect-fmt:` で折り返しを誘発しない。原則：再フォーマット出力はバイト単位同一、常に有効な SQL を生成 — stability-check でテスト。（[Ruff black deviations](https://docs.astral.sh/ruff/formatter/black/)）

### 4. 埋め込み言語フォーマット — `biome_js_formatter` 呼び出し＋ルート再インデント
**`$$ … $$` の JS 本体は `biome_js_formatter` を再利用；本体を単独でフォーマットし、配置カラムに全体を再インデント。** Biome は既に埋め込み CSS-in-JS スニペットフォーマットを提供しており、JS も同様にクレートレベルエントリが存在。*理由：*JS フォーマッタを再実装しない。（[Biome embedded CSS-in-JS](https://github.com/biomejs/biome/commit/bc0e8b47a276efabb0b76169d13dfc9d5325953f), [Biome v2.4 embedded snippets](https://biomejs.dev/blog/biome-v2-4/), [issue #3334](https://github.com/biomejs/biome/issues/3334)）
- 再インデント：Prettier のパターンは埋め込み位置で `markAsRoot` で新インデントルート設定、`dedentToRoot`/`literalline` で埋め込みDocをそのルート相対でインデント、`lineSuffixBoundary` でコメントが「コード部分を越えない」ように。SQL Doc に JS Doc をスプライスする際にこれを模倣。（[Prettier commands.md markAsRoot/dedentToRoot](https://github.com/prettier/prettier/blob/main/commands.md)）
- ドルクオート本体はハイライト用インジェクション/葉として扱う（`LANGUAGE`句から `@injection.content`＋`@injection.language`）、tree-sitter の SQL-in-JS と同様。（[tree-sitter syntax highlighting / injections](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html)）
- 同様に後で Python/Java/Scala UDF 本体にも拡張。常に「as-is / verbatim」フォールバックを用意し、パース不能でもフォーマッタが全体性を保つ。

### 5. ハイライト — 段階的：LSP セマンティックトークン → TextMate → tree-sitter
**この順で出荷。** *理由：*既にパーサーがあるので最も正確なルートが無料；tree-sitter は別のメンテ重い文法で、エディタエコシステム到達が目的、フォーマッタ品質ではない。
1. **LSP セマンティックトークン**（CST から `textDocument/semanticTokens`）—最も正確、文法追加不要、VS Code/Neovim/Zed/Helix で動作。（[tree-sitter highlighting overview for comparison](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html)）
2. **TextMate 文法**—LSP が動いていない VS Code/web/GitHub スタイルハイライトの安価な汎用フォールバック。
3. **tree-sitter 文法＋`highlights.scm`/`injections.scm`**—ネイティブ Neovim/Helix/Zed や GitHub.com ハイライト＋コードナビが必要な場合のみ（後者は Linguist エントリと crates.io 公開パーサーも必要）。これは*第二*のパーサーで、フォーマッタとは別。（[tree-sitter code navigation](https://tree-sitter.github.io/tree-sitter/4-code-navigation.html)）
- ソースが何であれ、Neovim のドット語彙（`@keyword`, `@keyword.operator`, `@string`, `@string.escape`, `@number`, `@function.call`, `@type.builtin`, `@operator`, `@comment`, `@punctuation.bracket`）でキャプチャ名を付け、テーマや将来の `highlights.scm` がエディタ間でドロップイン可能に。（[Neovim treesitter highlight groups](https://neovim.io/doc/user/treesitter.html), [DerekStride/tree-sitter-sql queries](https://github.com/DerekStride/tree-sitter-sql)）
- **Topiary をフォーマッタエンジンとして採用しない：**その softline はノードが >1 入力行かどうかのみで解決（幅予算なし）、トークン圧縮もあり。キャプチャセットのみ意思決定チェックリストとして借用。（[Topiary](https://github.com/tweag/topiary)）

### 6. テスト — スナップショット＋冪等性＋往復＋適合性コーパス
**Ruff のテストスタックを丸ごと採用。**（[Ruff formatter CONTRIBUTING](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_formatter/CONTRIBUTING.md)）
- **`insta` によるスナップショットテスト**（`resources/test/fixtures`、sqlparser-rs の `tests/sqlparser_snowflake.rs` SQL 文字列を入力として活用、Apache-2.0）。（[sqlparser snowflake tests](https://github.com/apache/datafusion-sqlparser-rs/blob/main/tests/sqlparser_snowflake.rs)）
- **往復/損失なしチェック：**CST の全トークンテキストを連結してソースをバイト単位で再現（rowan は正しく構築すれば保証）。（[RA syntax book](https://rust-analyzer.github.io/book/contributing/syntax.html)）
- **冪等性 `--stability-check`：**2回フォーマットしバイト同一性を検証；パニックや無効SQL出力も捕捉（sqlfluff のループ型fixはここで失敗 — 非冪等[#2134](https://github.com/sqlfluff/sqlfluff/issues/2134)）。（[Ruff stability](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_formatter/CONTRIBUTING.md)）
- **適合性ハーネス** `snow_dev format-dev`：実際の Snowflake SQL コーパス（dbt プロジェクト、Snowflake ドキュメント/サンプルクエリ）に対し、ファイルごとに**類似度＝neutral / (neutral + removed)** 差分スコアを CI ゲートとして報告。Ruff は Black と >99.9% 行単位同一を報告。（[Ruff format-dev / similarity](https://github.com/astral-sh/ruff/blob/main/crates/ruff_python_formatter/CONTRIBUTING.md)）
- 小規模な**ファズ**ターゲット（ランダム/変異SQL）を追加し、パーサーが決してパニックせず、出力が常に有効＋冪等であることを検証。

### 7. パフォーマンス — インターニング、シングルパス、並列化、インクリメンタル
- **アリーナ/インターニング：**`GreenNodeBuilder` 全体で1つの `NodeCache`（FxHash）を共有しトークンを重複排除；識別子をインターン。rowan の構造共有で安価。（[rowan node_cache](https://github.com/rust-analyzer/rowan/blob/master/src/green/node_cache.rs)）
- **シングルパス＋ゼロコピー：**テキストコピーせず `SourceCodeSlice` を出力；DocエンジンはCST1回歩き＋プリンタ1回（sqlfluff の収束ループはデフォルト10回）。（[Ruff format_element.rs](https://github.com/astral-sh/ruff/blob/main/crates/ruff_formatter/src/format_element.rs), [sqlfluff loop-limit #5325](https://github.com/sqlfluff/sqlfluff/issues/5325)）
- **並列化：**ファイルごとにスレッド（rayon）でフォーマット；各ファイルは独立。これ＋Rust が Python sqlfluff（大規模ファイルで数分〜数時間；自身もRust化中）に対する目玉の勝利。（[sqlfluff 4.0 Rust](https://github.com/sqlfluff/sqlfluff/releases/tag/4.0.0)）
- **LSP向けインクリメンタル：**rowan グリーンツリー再利用で、前ツリーをパッチしキー入力ごとに高速再解析 — イベント/ビルダー経路は編集フレンドリーに（v1フォーマッタは全ファイルだが）。（[RA architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)）
- **純Rust / Wasm圧力：**フォーマッタ/LSPパーサースタックはCベースエディタ文法から独立。tree-sitter は並行エコシステム資産でよいが、ブラウザ、Chrome拡張、将来のWebプレイグラウンドは純Rust CST経路に依存し、パッケージングを小さく予測可能に。

## 今すぐ適用 vs 後回しチェックリスト

**今（レキサー完成済み；パーサー/フォーマッタ未着手）：**
1. `SyntaxKind`（トークン＋ノード）＋`SnowLang` rowan `Language`＋`T!` マクロを定義。
2. パーサーを**イベントエミッタ**として構築（`Marker`/`complete`/`abandon`/`precede`, `TokenSet` リカバリ, 燃料ガード, 「決して失敗しない」）、`GreenNodeBuilder` にトリビアを付与。バイト単位往復を検証。
3. 汎用的な**Doc-IRエンジン**（`FormatElement`, `Format`/`FormatRule`, `group`/`indent`/`line`/`best_fitting`, `line_suffix`）をSQLルール前に立ち上げ。
4. コメント付与（先頭/末尾/ぶら下がり）＋**魔法の末尾カンマ**を実装。
5. `insta` スナップショット＋`--stability-check` を初日から導入；sqlparser-rs Snowflake テストからフィクスチャを種まき。
6. **意見先行、ほぼゼロ設定**（`line-length`, 必要なら `keyword-case`）。

**後回し：**
7. **sqlfluff `dialect_snowflake.py`** をチェックリストとして Snowflake 文法カバレッジ拡張（QUALIFY, 半構造化 `:`/`[]`, PIVOT/UNPIVOT, FLATTEN, MATCH_RECOGNIZE, COPY INTO, CREATE WAREHOUSE/STREAM/TASK）。
8. 埋め込み `$$…$$` JS を `biome_js_formatter`＋ルート再インデントで処理；verbatim フォールバック。
9. LSP **セマンティックトークン**ハイライト；次に**TextMate**文法。
10. `snow_dev format-dev` **類似度スコア**適合性ゲートを実コーパスで運用。
11. オプション：公式仕様由来パーサー/適合性ジェネレータ；ungrammar コード生成；dbt/Jinja テンプレートを sqlfluff 型リテラルアンカー付きスライスマップで；Neovim/GitHub 用 tree-sitter 文法。
12. オプション LSP リントレーン：DB接続不要のルール診断（巨大な `IN` リスト、スキーマメタデータ提供時の疑わしい nullable JOIN、カタログスナップショット利用時のオブジェクト参照チェック）。