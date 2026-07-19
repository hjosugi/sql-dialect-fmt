#!/usr/bin/env python3
"""Validate store artwork dimensions and package references without third-party modules."""

from __future__ import annotations

import json
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PNG_ASSETS = {
    "docs/store-assets/chrome/store-icon-128.png": (128, 128, True),
    "docs/store-assets/chrome/screenshot-formatter-1280x800.png": (1280, 800, False),
    "docs/store-assets/chrome/screenshot-options-1280x800.png": (1280, 800, False),
    "docs/store-assets/chrome/small-promo-440x280.png": (440, 280, False),
    "docs/store-assets/chrome/marquee-promo-1400x560.png": (1400, 560, False),
    "docs/store-assets/chrome/youtube-thumbnail-1280x720.png": (1280, 720, False),
    "editors/images/icon.png": (256, 256, True),
    "editors/images/syntax-highlighting.png": (1280, 800, False),
    "extensions/chrome/images/icon16.png": (16, 16, True),
    "extensions/chrome/images/icon32.png": (32, 32, True),
    "extensions/chrome/images/icon48.png": (48, 48, True),
    "extensions/chrome/images/icon128.png": (128, 128, True),
}

CSS_TOKEN_GROUPS = {
    "extensions/chrome/src/tokens.css": (
        "extensions/chrome/src/content.css",
        "extensions/chrome/src/options.css",
    ),
    "docs-site/theme/tokens.css": ("docs-site/theme/playground.css",),
    "docs/store-assets/source/tokens.css": (
        "docs/store-assets/source/chrome-formatter-demo.html",
        "docs/store-assets/source/chrome-options-demo.html",
        "docs/store-assets/source/demo-end.html",
        "docs/store-assets/source/demo-title.html",
        "docs/store-assets/source/vscode-syntax-demo.html",
    ),
}

CSS_VARIABLE_DEFINITION = re.compile(r"(--sdf-[a-z0-9-]+)\s*:")
CSS_VARIABLE_REFERENCE = re.compile(r"var\((--sdf-[a-z0-9-]+)")
RAW_FONT_SHORTHAND = re.compile(
    r"\bfont\s*:\s*[^;]*(?:\d+(?:\.\d+)?(?:px|rem)|monospace|sans-serif)",
    re.IGNORECASE,
)


def read_png(path: Path) -> tuple[int, int, bool]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ValueError("not a PNG with an IHDR header")
    width, height = struct.unpack(">II", data[16:24])
    # Indexed PNGs may carry transparency in a tRNS chunk instead of using RGBA color type 6.
    has_alpha = data[25] in (4, 6) or b"tRNS" in data
    return width, height, has_alpha


def validate_pngs() -> None:
    for relative, expected in PNG_ASSETS.items():
        path = ROOT / relative
        actual = read_png(path)
        expected_width, expected_height, requires_alpha = expected
        if actual[:2] != (expected_width, expected_height):
            raise ValueError(f"{relative}: expected {expected_width}x{expected_height}, got {actual[:2]}")
        if requires_alpha and not actual[2]:
            raise ValueError(f"{relative}: expected an alpha channel")


def validate_manifests() -> None:
    vscode = json.loads((ROOT / "editors/package.json").read_text())
    if vscode.get("icon") != "images/icon.png":
        raise ValueError("editors/package.json: icon must reference images/icon.png")
    packaged = set(vscode.get("files", []))
    for required in ("images/icon.png", "images/syntax-highlighting.png"):
        if required not in packaged:
            raise ValueError(f"editors/package.json: files must include {required}")
    if vscode.get("main") != "./dist/extension.js":
        raise ValueError("editors/package.json: main must reference the bundled extension")
    if "dist/extension.js" not in packaged or "node_modules/**" in packaged:
        raise ValueError("editors/package.json: package the bundle, not node_modules")
    if "vscode-languageclient" not in vscode.get("devDependencies", {}):
        raise ValueError("editors/package.json: devDependencies must include vscode-languageclient")
    if "esbuild" not in vscode.get("devDependencies", {}):
        raise ValueError("editors/package.json: devDependencies must include esbuild")

    chrome = json.loads((ROOT / "extensions/chrome/manifest.json").read_text())
    expected_icons = {size: f"images/icon{size}.png" for size in ("16", "32", "48", "128")}
    if chrome.get("icons") != expected_icons:
        raise ValueError("Chrome manifest top-level icons do not match packaged artwork")
    if chrome.get("action", {}).get("default_icon") != expected_icons:
        raise ValueError("Chrome manifest action icons do not match packaged artwork")
    content_css = chrome.get("content_scripts", [{}])[0].get("css")
    if content_css != ["src/tokens.css", "src/content.css"]:
        raise ValueError("Chrome manifest must load tokens.css before content.css")


def validate_css_tokens() -> None:
    for token_relative, consumer_relatives in CSS_TOKEN_GROUPS.items():
        token_path = ROOT / token_relative
        token_text = token_path.read_text()
        definitions = set(CSS_VARIABLE_DEFINITION.findall(token_text))
        if not any(name.startswith("--sdf-font-size-") for name in definitions):
            raise ValueError(f"{token_relative}: missing font-size tokens")
        if not any(name.startswith("--sdf-font-family-") for name in definitions):
            raise ValueError(f"{token_relative}: missing font-family tokens")

        for consumer_relative in consumer_relatives:
            consumer_text = (ROOT / consumer_relative).read_text()
            missing = set(CSS_VARIABLE_REFERENCE.findall(consumer_text)) - definitions
            if missing:
                names = ", ".join(sorted(missing))
                raise ValueError(f"{consumer_relative}: undefined CSS tokens: {names}")

            for line_number, line in enumerate(consumer_text.splitlines(), start=1):
                if "font-size:" in line and "font-size: var(--sdf-font-size-" not in line:
                    raise ValueError(
                        f"{consumer_relative}:{line_number}: font-size must use an --sdf token"
                    )
                if "font-family:" in line and "font-family: var(--sdf-font-family-" not in line:
                    raise ValueError(
                        f"{consumer_relative}:{line_number}: font-family must use an --sdf token"
                    )
                if RAW_FONT_SHORTHAND.search(line):
                    raise ValueError(
                        f"{consumer_relative}:{line_number}: numeric font shorthand bypasses tokens"
                    )

    options = (ROOT / "extensions/chrome/options.html").read_text()
    if '<link rel="stylesheet" href="src/tokens.css">' not in options:
        raise ValueError("Chrome options page must load src/tokens.css")

    for consumer_relative in CSS_TOKEN_GROUPS["docs/store-assets/source/tokens.css"]:
        source = (ROOT / consumer_relative).read_text()
        if '<link rel="stylesheet" href="tokens.css">' not in source:
            raise ValueError(f"{consumer_relative}: must load shared tokens.css")

    book = (ROOT / "docs-site/book.toml").read_text()
    expected = 'additional-css = ["theme/tokens.css", "theme/playground.css"]'
    if expected not in book:
        raise ValueError("docs-site/book.toml must load tokens.css before playground.css")


def validate_copy() -> None:
    privacy = (ROOT / "docs/PRIVACY.md").read_text()
    for phrase in ("Databricks", "chrome.storage.sync", "does not store SQL text"):
        if phrase not in privacy:
            raise ValueError(f"docs/PRIVACY.md: missing {phrase!r}")
    runbook = (ROOT / "docs/STORE_PUBLISHING.md").read_text()
    if "v1.0.0" in runbook or "version=1.0.0" in runbook:
        raise ValueError("docs/STORE_PUBLISHING.md still contains a v1.0.0 first-publish example")


def validate_video() -> None:
    relative = "docs/store-assets/chrome/demo-video-1280x720.mp4"
    path = ROOT / relative
    if not path.is_file() or path.stat().st_size < 100_000:
        raise ValueError(f"{relative}: missing or unexpectedly small")
    ffprobe = shutil.which("ffprobe")
    if ffprobe is None:
        print("ffprobe not found; skipped codec-level video validation")
        return
    result = subprocess.run(
        [
            ffprobe,
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_name,width,height,pix_fmt",
            "-of",
            "json",
            str(path),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    video = next(stream for stream in metadata["streams"] if stream.get("width"))
    if (video.get("codec_name"), video.get("width"), video.get("height"), video.get("pix_fmt")) != (
        "h264",
        1280,
        720,
        "yuv420p",
    ):
        raise ValueError(f"{relative}: unexpected video stream {video}")
    duration = float(metadata["format"]["duration"])
    if not 10 <= duration <= 60:
        raise ValueError(f"{relative}: expected a 10-60 second demo, got {duration}s")


def main() -> int:
    try:
        validate_pngs()
        validate_manifests()
        validate_css_tokens()
        validate_copy()
        validate_video()
    except (FileNotFoundError, ValueError, subprocess.CalledProcessError, StopIteration) as error:
        print(f"store asset validation failed: {error}", file=sys.stderr)
        return 1
    print(
        f"store asset validation ok: {len(PNG_ASSETS)} PNGs, 1 demo video, and shared CSS tokens"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
