#!/usr/bin/env bash
set -euo pipefail

ZIP_PATH="${1:-}"
if [ -z "$ZIP_PATH" ] || [ ! -f "$ZIP_PATH" ]; then
  echo "usage: scripts/publish-chrome-webstore.sh PATH_TO_chrome.zip" >&2
  exit 2
fi

for name in \
  CHROME_PUBLISHER_ID \
  CHROME_EXTENSION_ID \
  CHROME_CLIENT_ID \
  CHROME_CLIENT_SECRET \
  CHROME_REFRESH_TOKEN
do
  if [ -z "${!name:-}" ]; then
    echo "::error::$name is required to publish the Chrome extension" >&2
    exit 1
  fi
done

json_get() {
  local key="$1"
  python3 -c 'import json, re, sys
key = sys.argv[1]
snake = re.sub("([A-Z])", r"_\1", key).lower()
data = json.load(sys.stdin)
print(data.get(key) or data.get(snake) or "")' "$key"
}

api() {
  curl -fsS \
    -H "Authorization: Bearer $access_token" \
    "$@"
}

token_response="$(
  curl -fsS https://oauth2.googleapis.com/token \
    -d client_id="$CHROME_CLIENT_ID" \
    -d client_secret="$CHROME_CLIENT_SECRET" \
    -d refresh_token="$CHROME_REFRESH_TOKEN" \
    -d grant_type=refresh_token
)"
access_token="$(json_get access_token <<< "$token_response")"

if [ -z "$access_token" ]; then
  echo "::error::OAuth token response did not include access_token" >&2
  exit 1
fi

item="publishers/$CHROME_PUBLISHER_ID/items/$CHROME_EXTENSION_ID"

echo "Uploading Chrome Web Store package for $item"
upload_response="$(
  api -X POST \
  -H "Content-Type: application/zip" \
  --data-binary "@$ZIP_PATH" \
  "https://chromewebstore.googleapis.com/upload/v2/$item:upload"
)"
upload_state="$(json_get uploadState <<< "$upload_response")"

for _ in $(seq 1 "${CHROME_UPLOAD_POLL_ATTEMPTS:-20}"); do
  case "$upload_state" in
    UPLOAD_SUCCESS)
      break
      ;;
    UPLOAD_IN_PROGRESS)
      echo "Upload is still processing; polling fetchStatus"
      sleep "${CHROME_UPLOAD_POLL_SECONDS:-15}"
      status_response="$(api "https://chromewebstore.googleapis.com/v2/$item:fetchStatus")"
      upload_state="$(json_get uploadState <<< "$status_response")"
      ;;
    *)
      echo "::error::Chrome Web Store upload failed or returned unexpected state: $upload_state" >&2
      echo "$upload_response" >&2
      exit 1
      ;;
  esac
done

if [ "$upload_state" = "UPLOAD_IN_PROGRESS" ]; then
  echo "::error::Chrome Web Store upload did not finish before timeout" >&2
  exit 1
fi

publish_body='{"publishType":"DEFAULT_PUBLISH","blockOnWarnings":true}'
if [ "${CHROME_SKIP_REVIEW:-false}" = "true" ]; then
  publish_body='{"publishType":"DEFAULT_PUBLISH","blockOnWarnings":true,"skipReview":true}'
fi

echo "Submitting Chrome Web Store package for review/publish"
api -X POST \
  -H "Content-Type: application/json" \
  -d "$publish_body" \
  "https://chromewebstore.googleapis.com/v2/$item:publish"
