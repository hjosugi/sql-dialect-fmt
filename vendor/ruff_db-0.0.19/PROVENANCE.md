# Ruff DB ZIP dependency compatibility patch

Upstream: https://crates.io/crates/ruff_db/0.0.19
Source archive SHA-256: `e40482cca0e879d0c4c403eb09170431cd8bac0eeeed6601c84a1ffc4d627f9c`. Original MIT license and source files are preserved.

The normalized Cargo manifest constraint for `zip` changes from 8.0 to 9.0.0.
`src/vendored.rs` adapts the new fallible `file_names()` API: invalid names are
omitted from directory listings and debug formatting returns a formatting error.
Valid entry names and filesystem path handling remain unchanged. No advisory is ignored.
This replaces the vulnerable ZIP dependency in workspace application builds.
The workspace-root patch does not propagate when publishing libraries to crates.io;
consumers of published libraries still need the upstream Ruff constraint fixed.

Verification: Ruff vendored-filesystem tests, workspace tests with embedded Python,
and OSV lockfile rescan. See the private security-audit archive for exact commands
and file-by-file upstream hashes.

The upstream archive's standalone `Cargo.lock` is omitted because Cargo ignores
it for this path dependency. The workspace `Cargo.lock` is the application
build input and resolves ZIP 9.0.0. The upstream file hash is retained in the
private audit provenance record.
