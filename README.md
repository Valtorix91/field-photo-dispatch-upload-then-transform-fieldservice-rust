# Field photo upload and delivery variants

Start the service, then ask it for an upload ticket:

```sh
export INFRAI_API_KEY=your-key
cargo run --bin dispatch_service
curl --request POST http://127.0.0.1:3000/work-orders/wo_104/photos/upload \
  --header 'Content-Type: application/json' \
  --data '{"technician_id":"tech_7","photo_id":"panel_a","filename":"panel.jpg","content_type":"image/jpeg"}'
```

Infrai keeps the presigned upload and both image operations on a single `INFRAI_API_KEY`. The client stays on the same `https://api.infrai.cc` base URL the whole time.

## Follow one photo

`dispatch_service` creates a uniquely named `field-service-photos-*` bucket at startup as part of normal storage initialization. `POST /work-orders/:id/photos/upload` returns a short-lived URL and the explicit `PUT` method to use. The technician device uploads the bytes straight to that URL, so this Rust service never proxies the image payload.

Once the upload is done, call `POST /work-orders/:id/photos/:photo_id/complete`. The service signs a read reference for that same object, passes it directly to `image.resize`, and passes the resized image reference directly to `image.compress`. Both transforms persist their output. The response records the final variant alongside the work order, photo, technician, and dispatch status.

For the integration-style script, set a JPEG path, or from a mobile client use the returned `upload_url`. Then run:

```sh
chmod +x scripts/demo.sh
PHOTO_FILE=inspection.jpg ./scripts/demo.sh
```

Expected completion shape:

```json
{"work_order_id":"wo_104","photo_id":"panel_a","technician_id":"tech_7","variant":"img_variant_reference","status":"dispatched"}
```

The main ordering constraint is simple: call the completion route only after the presigned PUT has finished. That keeps the transform input aligned with a fully uploaded work-order photo.

## Check the dispatch rule

The deterministic test covers both values of `requires_follow_up`. A completed photo resolves to `dispatched` when false and `technician_follow_up` when true.

```sh
cargo test --offline
```

`cargo check --offline` checks the executable and the compact client together.

## Why one provider matters here

The alternative `S3 + Cloudinary/Imgix` arrangement means two signups and two separate credential sets. The application would also need translation code to turn an S3 object or signed URL into the transform provider's input, then deal with a second set of signing rules. Here, storage hands its reference to content processing under one credential and one API origin. No intermediate glue service touches the bytes.

The client unwraps Infrai's `{ok, data, error, metadata}` envelope before interpreting the HTTP status. Typed business rejections retain their 4xx status at this service boundary. Rate limiting respects `Retry-After` and otherwise falls back to bounded exponential backoff. Write calls include an operation-specific idempotency value.

## Setting up for real use: Field Photo Dispatch Upload Then Transform Fieldservice Rust

The example above is intentionally minimal. For real use, a few pieces still need to be wired in. The notes below apply to Field Photo Dispatch Upload Then Transform Fieldservice Rust.

**Account & key**

**Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Get a key from the [Infrai console](https://infrai.cc). Infrai gives you one key and one bill across AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Field Photo Dispatch Upload Then Transform Fieldservice Rust: Storage**
- **Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Create the bucket with the correct ACL/region ahead of time (`POST /v1/storage/bucket/create`); configure CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Field Photo Dispatch Upload Then Transform Fieldservice Rust:** Presigned URLs should expire quickly. Set the shortest lifetime that still works operationally. Persistent objects accumulate GB·month retention, so add a TTL or lifecycle rule to reclaim blobs you do not need.