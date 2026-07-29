"""Tests for scripts/release.sh argument handling, preflight, and dry-run plan.

The script drives an irreversible action (a tag push), so the contract that
matters is: it refuses bad input, it refuses an unsafe repository state, and
`--dry-run` mutates nothing. Every case here runs against a throwaway git
repository, never the working tree, and never reaches the network.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "release.sh"


def git(*args: str, cwd: Path) -> None:
    subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


class ReleaseScriptTests(unittest.TestCase):
    def run_script(self, *args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", str(SCRIPT), *args],
            cwd=cwd or ROOT,
            capture_output=True,
            text=True,
        )

    # ---- argument handling ----

    def test_help_exits_zero_and_documents_the_steps(self) -> None:
        result = self.run_script("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage: scripts/release.sh", result.stdout)
        self.assertIn("--via-ci", result.stdout)
        self.assertIn("--dry-run", result.stdout)

    def test_missing_version_is_a_usage_error(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 2)

    def test_rejects_non_semver_versions(self) -> None:
        for bad in ["1.2", "1.2.3.4", "latest", "v", "1.2.x"]:
            with self.subTest(version=bad):
                result = self.run_script(bad)
                self.assertEqual(result.returncode, 1)
                self.assertIn("not a semver version", result.stderr)

    def test_rejects_unknown_options_and_extra_arguments(self) -> None:
        unknown = self.run_script("1.21.0", "--nope")
        self.assertEqual(unknown.returncode, 1)
        self.assertIn("unknown option", unknown.stderr)

        extra = self.run_script("1.21.0", "2.0.0")
        self.assertEqual(extra.returncode, 1)
        self.assertIn("unexpected extra argument", extra.stderr)

    def test_publish_crates_requires_via_ci(self) -> None:
        result = self.run_script("1.21.0", "--publish-crates")
        self.assertEqual(result.returncode, 1)
        self.assertIn("--publish-crates only applies with --via-ci", result.stderr)

    # ---- preflight, against a scratch repository ----

    def make_repo(self) -> Path:
        """A minimal repo that satisfies the script up to the version bump.

        Scratch files for a test go in the repo's *parent*, never inside it —
        an untracked file in the repo trips the dirty-worktree preflight.
        """
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp, ignore_errors=True)

        repo = tmp / "repo"
        (repo / "scripts").mkdir(parents=True)
        shutil.copy(SCRIPT, repo / "scripts" / "release.sh")

        # Stand-ins for the real tools: the preflight only needs a version, and
        # --dry-run must never invoke the rest.
        (repo / "scripts" / "workspace-version.sh").write_text(
            "#!/usr/bin/env bash\necho 1.20.0\n"
        )
        for name in ("update-version.py", "check-version-consistency.py"):
            (repo / "scripts" / name).write_text("#!/usr/bin/env python3\n")
        (repo / "scripts" / "package-extensions.sh").write_text("#!/usr/bin/env bash\n")
        for path in (repo / "scripts").iterdir():
            path.chmod(0o755)

        (repo / "README.md").write_text("scratch\n")
        git("init", "-q", "-b", "main", cwd=repo)
        git("config", "user.email", "test@example.com", cwd=repo)
        git("config", "user.name", "Test", cwd=repo)
        git("add", "-A", cwd=repo)
        git("commit", "-qm", "initial", cwd=repo)
        return repo

    def run_in_repo(
        self, repo: Path, *args: str, path_prefix: Path | None = None, path: str | None = None
    ) -> subprocess.CompletedProcess[str]:
        env = dict(os.environ)
        # Keep a stray user gitconfig from renaming the default branch.
        env["GIT_CONFIG_GLOBAL"] = str(repo / ".gitconfig-absent")
        if path is not None:
            env["PATH"] = path
        elif path_prefix is not None:
            env["PATH"] = f"{path_prefix}{os.pathsep}{env['PATH']}"
        return subprocess.run(
            ["bash", str(repo / "scripts" / "release.sh"), *args],
            cwd=repo,
            capture_output=True,
            text=True,
            env=env,
        )

    def make_stub_gh(self, repo: Path) -> Path:
        """A directory holding only a no-op `gh`, outside the repo."""
        bin_dir = repo.parent / "fakebin"
        bin_dir.mkdir(exist_ok=True)
        stub = bin_dir / "gh"
        stub.write_text("#!/usr/bin/env bash\nexit 0\n")
        stub.chmod(0o755)
        return bin_dir

    def make_path_without_gh(self, repo: Path) -> str:
        """A PATH with the tools the script needs, but no `gh` on it."""
        bin_dir = repo.parent / "minimalbin"
        bin_dir.mkdir(exist_ok=True)
        for tool in ("bash", "git", "python3", "env", "grep", "dirname", "printf"):
            resolved = shutil.which(tool)
            if resolved:
                link = bin_dir / tool
                if not link.exists():
                    link.symlink_to(resolved)
        self.assertIsNone(shutil.which("gh", path=str(bin_dir)), "stub PATH must not expose gh")
        return str(bin_dir)

    def test_broken_path_fails_loudly_instead_of_using_the_wrong_tree(self) -> None:
        # Without `dirname`, ROOT_DIR collapses to "/" and the run would target
        # whatever tree happens to be there. It must refuse instead.
        repo = self.make_repo()
        bin_dir = repo.parent / "nodirname"
        bin_dir.mkdir()
        for tool in ("bash", "git", "python3", "env", "grep"):
            resolved = shutil.which(tool)
            if resolved:
                (bin_dir / tool).symlink_to(resolved)

        result = self.run_in_repo(repo, "1.21.0", "--dry-run", path=str(bin_dir))
        self.assertEqual(result.returncode, 1)
        self.assertIn("cannot locate the repository root", result.stderr)

    def test_refuses_a_dirty_working_tree(self) -> None:
        repo = self.make_repo()
        (repo / "README.md").write_text("dirty\n")
        result = self.run_in_repo(repo, "1.21.0", "--dry-run")
        self.assertEqual(result.returncode, 1)
        self.assertIn("working tree is dirty", result.stderr)

    def test_refuses_a_non_release_branch(self) -> None:
        repo = self.make_repo()
        git("checkout", "-qb", "feature", cwd=repo)
        result = self.run_in_repo(repo, "1.21.0", "--dry-run")
        self.assertEqual(result.returncode, 1)
        self.assertIn("expected 'main'", result.stderr)

    def test_branch_override_accepts_another_release_branch(self) -> None:
        repo = self.make_repo()
        git("checkout", "-qb", "release-1.x", cwd=repo)
        result = self.run_in_repo(repo, "1.21.0", "--dry-run", "--branch", "release-1.x")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_refuses_a_version_that_is_already_tagged(self) -> None:
        repo = self.make_repo()
        git("tag", "v1.21.0", cwd=repo)
        result = self.run_in_repo(repo, "1.21.0", "--dry-run")
        self.assertEqual(result.returncode, 1)
        self.assertIn("already exists locally", result.stderr)

    def test_leading_v_is_accepted_and_normalized(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(repo, "v1.21.0", "--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("v1.21.0", result.stdout)
        self.assertNotIn("vv1.21.0", result.stdout)

    # ---- the dry-run plan ----

    def test_dry_run_changes_nothing_and_plans_every_step(self) -> None:
        repo = self.make_repo()
        before = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=repo, capture_output=True, text=True
        ).stdout

        result = self.run_in_repo(repo, "1.21.0", "--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)

        for planned in [
            "update-version.py 1.21.0 --changelog",
            "cargo test --workspace",
            "cargo clippy --workspace --all-targets -- -D warnings",
            "package-extensions.sh 1.21.0",
            "git commit -am release: v1.21.0",
            "git tag v1.21.0",
            "git push origin v1.21.0",
        ]:
            self.assertIn(planned, result.stdout, f"missing planned step: {planned}")

        # Nothing ran: no new commit, no tag, no dirty files.
        after = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=repo, capture_output=True, text=True
        ).stdout
        self.assertEqual(before, after)
        tags = subprocess.run(
            ["git", "tag"], cwd=repo, capture_output=True, text=True
        ).stdout
        self.assertEqual(tags.strip(), "")
        status = subprocess.run(
            ["git", "status", "--porcelain"], cwd=repo, capture_output=True, text=True
        ).stdout
        self.assertEqual(status.strip(), "")

    def test_no_gate_skips_the_slow_checks_but_keeps_version_consistency(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(repo, "1.21.0", "--dry-run", "--no-gate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("cargo test", result.stdout)
        self.assertNotIn("cargo clippy", result.stdout)
        self.assertIn("check-version-consistency.py v1.21.0", result.stdout)

    def test_no_push_stops_before_pushing_and_prints_the_manual_finish(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(repo, "1.21.0", "--dry-run", "--no-push")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("+ git push", result.stdout)
        self.assertNotIn("[dry-run] git push", result.stdout)
        self.assertIn("git push origin main && git push origin v1.21.0", result.stdout)

    def test_via_ci_dispatches_instead_of_tagging(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(
            repo,
            "1.21.0",
            "--dry-run",
            "--via-ci",
            path_prefix=self.make_stub_gh(repo),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("gh workflow run release.yml", result.stdout)
        self.assertIn("version=v1.21.0", result.stdout)
        self.assertIn("publish_crates=false", result.stdout)
        # A dispatch must not also create a local tag; the workflow makes it.
        self.assertNotIn("git tag v1.21.0", result.stdout)

    def test_via_ci_without_gh_is_refused_before_any_mutation(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(
            repo,
            "1.21.0",
            "--dry-run",
            "--via-ci",
            path=self.make_path_without_gh(repo),
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("needs the GitHub CLI", result.stderr)

    def test_via_ci_forwards_publish_crates(self) -> None:
        repo = self.make_repo()
        result = self.run_in_repo(
            repo,
            "1.21.0",
            "--dry-run",
            "--via-ci",
            "--publish-crates",
            path_prefix=self.make_stub_gh(repo),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("publish_crates=true", result.stdout)


if __name__ == "__main__":
    unittest.main()
