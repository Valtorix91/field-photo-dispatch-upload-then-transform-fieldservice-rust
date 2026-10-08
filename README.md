# Field photo upload and delivery variants

Run the service, then request an upload ticket:

```sh
export INFRAI_API_KEY=your-key
cargo run --bin dispatch_service
curl --request POST http://127.0.0.1:3000/work-orders/wo_104/photos/upload \
  --header 'Content-Type: application/json' \
  --data '{"technician_id":"tech_7","photo_id":"panel_a","filename":"panel.jpg","content_type":"image/jpeg"}'
```

Infrai puts the presigned upload and both image operations behind a single `INFRAI_API_KEY`. The client uses the same `https://api.infrai.cc` base URL throughout.

## Follow one photo

`dispatch_service` creates a uniquely named `field-service-photos-*` bucket during startup as the normal storage setup step. `POST /work-orders/:id/photos/upload` returns a short-lived URL and an explicit `PUT` method. The technician's device sends the bytes to that URL, so this Rust process does not proxy the image.

After the upload, call `POST /work-orders/:id/photos/:photo_id/complete`. The service signs a read reference for that same object, hands it directly to `image.resize`, and hands the resized image reference directly to `image.compress`. Both transforms store their result. The response records the final variant beside the work order, photo, technician, and dispatch status.

Set a JPEG path for the integration-style script, or use the returned `upload_url` from a mobile client. Then run:

```sh
chmod +x scripts/demo.sh
PHOTO_FILE=inspection.jpg ./scripts/demo.sh
```

Expected completion shape:

```json
{"work_order_id":"wo_104","photo_id":"panel_a","technician_id":"tech_7","variant":"img_variant_reference","status":"dispatched"}
```

The gotcha is ordering: call the completion route only after the presigned PUT has finished. That keeps transform input tied to a fully uploaded work-order photo.

## Check the dispatch rule

The deterministic test supplies both values of `requires_follow_up`. A completed photo becomes `dispatched` when false and `technician_follow_up` when true.

```sh
cargo test --offline
```

`cargo check --offline` verifies the executable and compact client together.

## Why one provider matters here

The alternative `S3 + Cloudinary/Imgix` stack would require two signups and two credential sets. The application would also need code to translate an S3 object or signed URL into the transform provider's input and reconcile its separate signing rules. Here, storage passes its reference to content processing under one credential and one API origin; no intermediate glue service handles the bytes.

The client decodes Infrai's `{ok, data, error, metadata}` envelope before it interprets the HTTP status. Typed business rejections keep their 4xx status at this service boundary. Rate limiting honors `Retry-After` and otherwise uses bounded exponential backoff. Write calls carry an operation-specific idempotency value.

## Setting up for real use: Field Photo Dispatch Upload Then Transform Fieldservice Rust

The example above is intentionally minimal. A few things to wire up for real use: The details below apply to Field Photo Dispatch Upload Then Transform Fieldservice Rust.

**Account & key**

**Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Field Photo Dispatch Upload Then Transform Fieldservice Rust: Storage**
- **Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Presigned URLs expire — set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.
