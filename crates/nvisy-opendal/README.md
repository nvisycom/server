# nvisy-opendal

Storage abstraction layer for the Nvisy platform using OpenDAL for unified access
to multiple cloud storage backends.

## Features

- **Unified API** - Single interface for multiple storage backends
- **Cloud-Native** - Support for major cloud storage providers
- **Async Operations** - Non-blocking I/O with Tokio runtime
- **Feature Flags** - Enable only the backends you need

## Supported Backends

| Backend | Feature Flag | Description |
|---------|--------------|-------------|
| Amazon S3 | `s3` | S3-compatible object storage |
| Google Cloud Storage | `gcs` | GCS bucket storage |
| Azure Blob Storage | `azblob` | Azure container storage |
| Google Drive | `gdrive` | Google Drive file storage |
| Dropbox | `dropbox` | Dropbox cloud storage |
| OneDrive | `onedrive` | Microsoft OneDrive storage |

## Usage

Enable the backends you need in `Cargo.toml`:

```toml
[dependencies]
nvisy-opendal = { path = "../nvisy-opendal", features = ["s3", "gcs"] }
```

Or enable all backends:

```toml
[dependencies]
nvisy-opendal = { path = "../nvisy-opendal", features = ["all-backends"] }
```

## Key Dependencies

- `opendal` - Unified data access layer for multiple storage services
- `tokio` - Async runtime for non-blocking I/O operations
- `jiff` - Modern date/time handling for file metadata
