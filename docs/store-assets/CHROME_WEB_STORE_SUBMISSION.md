<!-- i18n: language-switcher -->
[English](CHROME_WEB_STORE_SUBMISSION.md) | [日本語](CHROME_WEB_STORE_SUBMISSION.ja.md)

# Chrome Web Store submission sheet

Last verified against the official Chrome Web Store listing and image documentation: 2026-07-11.

This sheet is the copy/paste source of truth for the initial listing and later updates.

## Store listing

**Product name**

```text
sql-dialect-fmt for SQL editors
```

**Summary**

```text
Format Snowflake and Databricks SQL directly in browser editors with sql-dialect-fmt.
```

**Detailed description**

```text
sql-dialect-fmt formats SQL in the active Snowflake Snowsight worksheet or Databricks SQL editor.

Run it from the floating editor button, the extension action button, or Alt+Shift+F. If a SQL range is selected, only that range is formatted. Otherwise the extension formats the whole active SQL editor.

Choose Snowflake or Databricks mode in the options page and set line width, indent width, and keyword casing. These formatter preferences can sync between signed-in Chrome profiles.

Formatting runs locally in the browser with the bundled WebAssembly build of sql-dialect-fmt. SQL text is not stored and is not sent to an external service.
```

**Category:** `Developer Tools`

**Language:** `English`

## Graphic assets

| Field | Upload |
| --- | --- |
| 128×128 store icon | `chrome/store-icon-128.png` |
| 1280×800 screenshot 1 | `chrome/screenshot-formatter-1280x800.png` |
| 1280×800 screenshot 2 | `chrome/screenshot-options-1280x800.png` |
| 440×280 small promo tile | `chrome/small-promo-440x280.png` |
| 1400×560 marquee tile | `chrome/marquee-promo-1400x560.png` |

**Localized promo video:** upload `chrome/demo-video-1280x720.mp4` to the release account's
YouTube channel as `Unlisted`, then paste the real YouTube watch URL. This URL cannot be created
from repository credentials and must not be replaced with a fake or non-YouTube URL.

Use `chrome/youtube-thumbnail-1280x720.png` as the optional YouTube thumbnail.

**YouTube title**

```text
sql-dialect-fmt for SQL editors — Chrome extension demo
```

**YouTube description**

```text
Format Snowflake Snowsight and Databricks SQL directly in the active browser editor with sql-dialect-fmt.

Run the formatter from the editor button, Chrome extension action, or Alt+Shift+F. Formatting runs locally with bundled WebAssembly. SQL text is not stored or sent to an external service.

Source and documentation: https://github.com/hjosugi/sql-dialect-fmt
Privacy policy: https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

- Visibility: `Unlisted`
- Audience: `No, it is not made for kids`
- Paid promotion: `No`
- Altered/synthetic content disclosure: answer according to the current YouTube form; the video
  contains product UI fixtures and generated brand artwork but no realistic people or events.

## Privacy practices

**Single purpose**

```text
Format SQL in the active Snowflake Snowsight worksheet or Databricks SQL editor using the bundled sql-dialect-fmt WebAssembly formatter.
```

**Permission justifications**

```text
activeTab: Used so the extension action and keyboard shortcut can run only after the user invokes the formatter in the active tab.

storage: Stores only formatter preferences such as SQL dialect, line width, indent width, and keyword casing using chrome.storage.sync. It does not store SQL text.

Host permissions for Snowflake/Snowsight and Databricks domains: Required to detect the active SQL editor and replace the selected SQL, or the whole editor contents, with formatted SQL.
```

**Remote code:** `No`

```text
No remote code is used. The Rust formatter is compiled to WebAssembly and bundled in the extension package. All JavaScript, CSS, and WebAssembly executed by the extension ships inside the package.
```

**Data collection:** select no data categories.

```text
The extension does not collect, transmit, sell, or share user data. SQL text is processed locally on demand and is not stored. Only formatter preferences are saved with chrome.storage.sync.
```

**Privacy policy URL**

```text
https://github.com/hjosugi/sql-dialect-fmt/blob/main/docs/PRIVACY.md
```

Check the required certifications that the data use complies with Chrome Web Store policies and
that user data is not sold or used outside the disclosed single purpose.

## Distribution

- Visibility: `Public`
- Regions: `All regions` unless the release owner has a legal reason to restrict distribution
- Pricing: `Free`

## Reviewer instructions

```text
1. Install the extension and open a supported Snowflake Snowsight or Databricks SQL editor.
2. Enter demo SQL such as: select customer_id,sum(amount) from orders group by customer_id
3. Focus the SQL editor.
4. Run the floating sql-dialect-fmt button, the extension action, or Alt+Shift+F.
5. Confirm the SQL is formatted in place.
6. Open the extension options and switch between Snowflake and Databricks or change line width, indent width, and keyword casing.

No account credentials are bundled with the extension. Reviewers need access to a supported editor page. Formatting runs locally and does not call a developer-operated server.
```

## Final dashboard checklist

- [ ] Upload the `v1.13.0` Chrome zip.
- [ ] Paste all listing copy exactly.
- [ ] Upload the icon, two screenshots, small promo, and optional marquee files.
- [ ] Upload the checked-in MP4 to YouTube and paste the real URL.
- [ ] Complete privacy answers and permission justifications.
- [ ] Set Public / All regions / Free.
- [ ] Save the draft and run through the reviewer instructions once.
- [ ] Submit for review.
