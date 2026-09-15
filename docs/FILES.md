# ARO File Storage

ARO stores file metadata in PostgreSQL and file bytes in an object store.

## Control Plane

PostgreSQL owns:

- `file_objects`: ownership, MIME, size, SHA-256, object key, status, scan status.
- `file_upload_sessions`: short-lived pending uploads.
- `file_links`: links from messages/runs/artifacts to files.
- `file_chunks`: extracted text chunks for BM25 and reindexing.
- `file_scan_jobs`: durable ClamAV scan queue with lease tokens and bounded retries.
- `file_index_jobs`: async indexing work queue, created only after a clean scan.

Audit and outbox events intentionally omit raw file bytes and original file names.

## Data Plane

`crates/aro-files` exposes `FileStorage` with:

- `LocalFileStorage` for offline tests and local filesystem-only development.
- `S3FileStorage` for MinIO and S3-compatible production storage.

Object keys are generated server-side as `organizations/{orgId}/files/{fileId}/object`; users never choose storage paths.

## Upload Flow

1. `POST /files/uploads` creates a pending `file_object` and upload session.
2. `PUT /files/uploads/{uploadId}/content` streams bytes to storage, recalculates SHA-256, sniffs MIME/magic bytes and enforces the size/MIME allowlist. The file remains `pending`.
3. The `aro-api worker` claims a scan job with a lease, retrieves the object, and sends it over the `clamd` INSTREAM protocol to a private antivirus service. A clean result makes the file `available`; a detection quarantines it; scanner failures retry with a bounded budget.
4. Messages can attach only `available` objects; file text is not copied into `messages.content`.

## Local Dev

Run `docker compose up -d postgres qdrant minio minio-init`.

To make uploads available in local development, run a private `clamd` service, set `ARO_CLAMAV_ADDRESS` to its `host:port` (for example `127.0.0.1:3310`), then run `cargo run -p aro-api -- worker`. Without it, files remain safely pending.

Default MinIO settings are in `.env.example`:

- `ARO_FILE_STORAGE_BACKEND=s3`
- `ARO_S3_ENDPOINT=http://127.0.0.1:9000`
- `ARO_S3_BUCKET=aro-files-dev`
- `AWS_ACCESS_KEY_ID=minioadmin`
- `AWS_SECRET_ACCESS_KEY=minioadmin`
