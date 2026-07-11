<!-- i18n: language-switcher -->
[English](ASSET_PROVENANCE.md) | [日本語](ASSET_PROVENANCE.ja.md)

# Store asset provenance

Last updated: 2026-07-11.

## Brand artwork

The icon, small promo tile, and marquee promo tile were created for this repository with the
built-in OpenAI image generation workflow. No external logo, screenshot, customer data, or stock
asset was supplied as input.

The icon prompt requested an original minimal emblem combining code brackets, aligned formatting
lines, and a snow crystal in deep teal, mint, and white, with no text, trademarks, or watermark.
It was generated on a flat magenta chroma-key background, converted to alpha with the imagegen
chroma-removal helper, and inspected at 256, 128, 48, 32, and 16 pixels.

The promo prompts used the generated emblem as their only reference and requested saturated
developer-tool artwork showing staggered abstract lines becoming aligned lines. They explicitly
prohibited text, SQL code, third-party logos, screenshots, and watermarks. The final assets were
cropped or resized to the exact Chrome Web Store dimensions and inspected at native and half size.

## Product screenshots

The Chrome and VS Code screenshots are deterministic browser renders of the HTML fixtures under
`source/`. They contain only fictional workspace names and demo SQL. The extension button, success
toast, preference fields, supported dialects, syntax scopes, and privacy statements match the
shipped source code.

## Demo video

`chrome/demo-video-1280x720.mp4` is a silent 16-second H.264/YUV420p presentation assembled from
the checked-in product screenshots and deterministic title/end cards. It contains no people,
voices, customer data, third-party logos, or copyrighted music. Four representative frames were
visually inspected after encoding.
