<!-- i18n: language-switcher -->
[English](README.md) | [日本語](README.ja.md)

# ストア提出資産

このディレクトリには、Chrome Web StoreおよびVS Code Marketplaceへの提出に使用されるレビュー済みの正確なサイズの資産が含まれています。

## Chrome Web Store インベントリ

| 資産 | 寸法 / フォーマット | ステータス |
| --- | --- | --- |
| `chrome/store-icon-128.png` | 128×128 PNG（アルファ付き） | 必須、準備完了 |
| `chrome/screenshot-formatter-1280x800.png` | 1280×800 PNG | 必須、準備完了 |
| `chrome/screenshot-options-1280x800.png` | 1280×800 PNG | オプションの2枚目のスクリーンショット、準備完了 |
| `chrome/small-promo-440x280.png` | 440×280 PNG | 必須、準備完了 |
| `chrome/marquee-promo-1400x560.png` | 1400×560 PNG | オプション、準備完了 |
| `chrome/demo-video-1280x720.mp4` | 1280×720 H.264、16秒 | 必須ソース、YouTubeにアップロードする準備完了 |
| `chrome/youtube-thumbnail-1280x720.png` | 1280×720 PNG | オプションのYouTubeサムネイル、準備完了 |

スクリーンショットは、`source/`のHTMLフィクスチャからレンダリングされています。デモSQLと一般的なSQLワークスペースを使用しているため、顧客データ、アカウント詳細、またはサードパーティ製品のアートワークは含まれていません。拡張機能のボタン、成功トースト、オプション、サポートされている方言、およびプライバシー声明は、出荷された拡張機能の動作と一致しています。各フィクスチャは`source/tokens.css`を共有しており、フォントファミリー、フォントサイズ、ウェイト、余白、角丸、ブランドカラーは個別のフィクスチャに再定義せず、このファイルで変更します。

アイコンとプロモーションタイルは、組み込みの画像生成ワークフローを使用して生成されました。最終的な資産は、正確な公式寸法にリサイズされ、ネイティブサイズおよび縮小されたアイコン/タイルサイズで視覚的に確認されました。プロモタイルには意図的にテキストは含まれていません。

すべてのダッシュボード値、プライバシー回答、レビュアー指示、および1つの外部YouTubeアップロードステップについては、[`CHROME_WEB_STORE_SUBMISSION.md`](CHROME_WEB_STORE_SUBMISSION.md)を参照してください。

`python3 scripts/check-store-assets.py`を実行して、すべてのPNG寸法、アイコンのアルファチャンネル、パッケージ参照、共有CSS token参照、プライバシー声明、およびデモビデオのコーデック、寸法、持続時間を確認してください。
