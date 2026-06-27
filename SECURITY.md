# Security Policy

sql-dialect-fmt is a developer tool that parses and formats source text. Please report
security issues privately when possible.

## Reporting

Do not open a public issue for a vulnerability. Use GitHub private vulnerability
reporting if it is enabled for the repository. If that is not available, contact
a maintainer privately and include:

- affected version or commit
- steps to reproduce
- impact
- whether the issue involves untrusted SQL input, generated code, editor
  integration, or packaging

## Scope

In scope:

- crashes or denial-of-service from crafted SQL input
- unsafe generated parser integration
- editor/LSP behavior that can execute unintended code
- supply-chain or release packaging problems

Out of scope:

- general SQL injection in downstream applications
- Snowflake account configuration issues unrelated to this tool
