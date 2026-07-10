class SqlDialectFmt < Formula
  desc "Opinionated SQL dialect formatter for Snowflake and Databricks SQL"
  homepage "https://github.com/hjosugi/sql-dialect-fmt"
  url "https://github.com/hjosugi/sql-dialect-fmt.git", tag: "v1.11.0"
  license "0BSD"
  head "https://github.com/hjosugi/sql-dialect-fmt.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/sql-dialect-fmt-cli")
  end

  test do
    (testpath/"query.sql").write "select 1 as id"
    assert_equal "SELECT 1 AS id;\n", shell_output("#{bin}/sql-dialect-fmt query.sql")
  end
end
