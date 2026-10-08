#!/bin/sh
set -eu

base="${SERVICE_URL:-http://127.0.0.1:3000}"
: "${PHOTO_FILE:?set PHOTO_FILE to a JPEG image}"
ticket=$(curl --fail --silent --show-error --request POST \
  "$base/work-orders/wo_104/photos/upload" \
  --header 'Content-Type: application/json' \
  --data '{"technician_id":"tech_7","photo_id":"panel_a","filename":"panel.jpg","content_type":"image/jpeg"}')
printf '%s\n' "$ticket"

curl --fail --silent --show-error --request PUT \
  "$(printf '%s' "$ticket" | sed -n 's/.*"upload_url":"\([^"]*\)".*/\1/p')" \
  --header 'Content-Type: image/jpeg' --data-binary "@$PHOTO_FILE"

curl --fail --silent --show-error --request POST \
  "$base/work-orders/wo_104/photos/panel_a/complete" \
  --header 'Content-Type: application/json' \
  --data '{"technician_id":"tech_7","requires_follow_up":false}'
printf '\n'
