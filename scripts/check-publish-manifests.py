#!/usr/bin/env python3
"""Reject registry-resolvable dependencies on private workspace crates."""

from __future__ import annotations

import pathlib
import sys
import tomllib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def load_manifest(path: pathlib.Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def dependency_tables(manifest: dict[str, Any]):
    for name in DEPENDENCY_TABLES:
        yield name, manifest.get(name, {})

    for target_name, target in manifest.get("target", {}).items():
        for name in DEPENDENCY_TABLES:
            yield f"target.{target_name}.{name}", target.get(name, {})


def package_name(dependency_name: str, specification: Any) -> str:
    if isinstance(specification, dict):
        return specification.get("package", dependency_name)
    return dependency_name


def main() -> int:
    root_manifest = load_manifest(ROOT / "Cargo.toml")
    workspace_dependencies = root_manifest["workspace"].get("dependencies", {})

    manifests = []
    for path in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        manifest = load_manifest(path)
        package = manifest["package"]
        publish = package.get("publish", True)
        manifests.append((path, manifest, publish is not False and publish != []))

    private_packages = {
        manifest["package"]["name"]
        for _, manifest, publishable in manifests
        if not publishable
    }
    errors: list[str] = []

    for path, manifest, publishable in manifests:
        if not publishable:
            continue

        relative_path = path.relative_to(ROOT)
        for table_name, dependencies in dependency_tables(manifest):
            for dependency_name, specification in dependencies.items():
                resolved_specification = specification
                resolved_name = package_name(dependency_name, specification)

                if isinstance(specification, dict) and specification.get("workspace") is True:
                    resolved_specification = workspace_dependencies.get(dependency_name, {})
                    resolved_name = package_name(dependency_name, resolved_specification)

                if resolved_name not in private_packages:
                    continue

                is_path_only_dev_dependency = (
                    table_name.endswith("dev-dependencies")
                    and isinstance(resolved_specification, dict)
                    and "path" in resolved_specification
                    and "version" not in resolved_specification
                    and "registry" not in resolved_specification
                    and "workspace" not in resolved_specification
                )
                if is_path_only_dev_dependency:
                    continue

                errors.append(
                    f"{relative_path}: [{table_name}] {dependency_name} resolves to private "
                    f"workspace package {resolved_name!r}; use a path-only dev-dependency"
                )

    if errors:
        print("invalid publishable Cargo manifests:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1

    publishable_count = sum(publishable for _, _, publishable in manifests)
    print(
        "publish manifest check ok: "
        f"{publishable_count} publishable crates, {len(private_packages)} private crates"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
