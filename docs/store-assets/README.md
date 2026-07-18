<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# Store submission assets

This directory contains the reviewed, exact-size assets used for the Chrome Web Store and VS Code
Marketplace submissions.

## Chrome Web Store inventory

| Asset | Dimensions / format | Status |
| --- | --- | --- |
| `chrome/store-icon-128.png` | 128×128 PNG with alpha | required, ready |
| `chrome/screenshot-formatter-1280x800.png` | 1280×800 PNG | required, ready |
| `chrome/screenshot-options-1280x800.png` | 1280×800 PNG | optional second screenshot, ready |
| `chrome/small-promo-440x280.png` | 440×280 PNG | required, ready |
| `chrome/marquee-promo-1400x560.png` | 1400×560 PNG | optional, ready |
| `chrome/demo-video-1280x720.mp4` | 1280×720 H.264, 16 seconds | required source, ready to upload to YouTube |
| `chrome/youtube-thumbnail-1280x720.png` | 1280×720 PNG | optional YouTube thumbnail, ready |

The screenshots are rendered from the HTML fixtures under `source/`. They use demo SQL and a
generic SQL workspace so no customer data, account details, or third-party product artwork is
included. The extension button, success toast, options, supported dialects, and privacy statements
match the shipped extension behavior.

The icon and promotional tiles were generated with the built-in image generation workflow. Final
assets were resized to the exact official dimensions and visually checked at native size and at
downscaled icon/tile sizes. The promo tiles intentionally contain no text.

See [`CHROME_WEB_STORE_SUBMISSION.md`](CHROME_WEB_STORE_SUBMISSION.md) for every dashboard value,
privacy answer, reviewer instruction, and the one external YouTube upload step.

Run `python3 scripts/check-store-assets.py` to verify every PNG dimension, icon alpha channel,
package reference, privacy statement, and the demo video's codec, dimensions, and duration.
