# Docker

Docker configuration for the Nvisy server.

## Infrastructure Requirements

Nvisy requires three external services:

**PostgreSQL 18+**. PostgreSQL serves as the primary data store for all
application state: accounts, workspaces, documents, connections, and file
metadata. It uses the `pgcrypto` and `pg_trgm` contrib extensions, both bundled
with the standard image. The recommended image is `postgres:18`.

**NATS 2.10+** with JetStream enabled. NATS handles pub/sub messaging for
real-time events and persistent job queues for asynchronous processing.
JetStream must be enabled with sufficient storage allocation: the default
configuration uses 1 GB of memory store and 10 GB of file store.

**An S3-compatible object store** for first-party blobs — uploaded files,
detection audits, redacted output, and avatars. The compose files use
[RustFS](https://rustfs.com) (MinIO-compatible, Apache-2.0); AWS S3 or any
S3-compatible service (MinIO, Cloudflare R2, …) works by pointing `S3_ENDPOINT`
at it. The configured `S3_BUCKET` must exist before the server starts — the
compose files provision it with a one-shot init container.

## Quick Start

### Development (infrastructure only)

Start PostgreSQL, NATS, and RustFS for local development:

```bash
docker compose -f docker-compose.dev.yml up -d
```

This starts the services with development defaults (`postgres:postgres`
credentials, JetStream enabled, RustFS with `rustfsadmin` credentials and the
`nvisy-dev` bucket auto-created). Then generate configuration and run the server
locally:

```bash
make generate-all   # .env, keys, migrations
cargo run --features dotenv --bin nvisy-server
```

The API documentation is available at:

- Scalar UI: `http://localhost:8080/api/scalar`
- OpenAPI JSON: `http://localhost:8080/api/openapi.json`

### Production

Build and run the complete stack:

```bash
cp .env.example .env
# Edit .env with production values
docker compose up -d --build
```

The production compose file starts every service on a private bridge network.
The server waits for the PostgreSQL, NATS, and RustFS health checks to pass —
and for the bucket-provisioning init container to finish — before starting.

## Services

| Service    | Port(s)    | Description                      |
| ---------- | ---------- | -------------------------------- |
| PostgreSQL | 5432       | Primary database                 |
| NATS       | 4222, 8222 | Message queue (JetStream)        |
| RustFS     | 9000, 9001 | S3-compatible blob store         |
| Server     | 8080       | Nvisy API                        |

## Configuration

All configuration is provided through environment variables. See
[`.env.example`](../.env.example) at the repository root for a complete
reference with defaults and descriptions.

## Key Generation

The server requires an Ed25519 keypair for JWT signing and a 32-byte key for
connection credential encryption. Generate both with:

```bash
make generate-keys
```

This produces three files: `private.pem`, `public.pem`, and `encryption.key`. In
production, store these securely and reference them via the environment
variables above.

## Container Image

The Dockerfile uses a multi-stage build:

1. **Planner:** generates a dependency recipe with cargo-chef
2. **Builder:** builds dependencies from the recipe (cached), then builds the
   server binary and strips it
3. **Runtime:** minimal Debian image with only the binary and runtime libraries

The runtime image runs as a non-root user (`nvisy`, UID 1000) and includes a
health check endpoint at `/health/`.

## NATS Configuration

The default NATS configuration (`nats/nats.conf`) enables JetStream with:

- 1 GB memory store for high-throughput streams
- 10 GB file store for persistent data
- 8 MB maximum payload size

Adjust these values based on expected workload. The memory store is used for
ephemeral streams; the file store is used for durable subscriptions and job
queues.

## Encryption at Rest

The server encrypts sensitive payloads at the application layer with
XChaCha20-Poly1305 under per-workspace keys before they reach storage — file
bytes, redacted output, analyzed documents, webhook signing secrets, and
policy/context definitions. The blob store only ever receives ciphertext; any
server-side encryption it offers is redundant defense-in-depth. This protects
those payloads even against a live read of the datastore.

For everything else on disk — NATS stream/consumer metadata and KV entries,
Postgres rows, backups — provision the data volumes on **encrypted storage**.
This adds no runtime overhead, keeps key management out of the datastores, and
covers the whole volume.

The persistent volumes to encrypt are `nats_data`, `postgres_data`, and
`rustfs_data`:

- **Cloud:** back the volumes with an encrypted block device (e.g. an encrypted
  EBS volume, or a cloud disk with default SSE enabled), or bind-mount them onto
  an encrypted filesystem.
- **Self-hosted:** place the Docker data root (or a bind mount for these
  volumes) on a LUKS-encrypted partition.

Volume encryption guards against stolen disks, snapshots, and backups. It does
not protect against a compromised running host — which is why the sensitive
payloads above are additionally encrypted at the application layer.

## Health Checks

All services expose health check endpoints:

| Service    | Endpoint                | Method   |
| ---------- | ----------------------- | -------- |
| Server     | `/health/`              | HTTP GET |
| PostgreSQL | `pg_isready`            | CLI      |
| NATS       | `/healthz` on port 8222 | HTTP GET |
| RustFS     | `/health` on port 9000  | HTTP GET |

The compose files configure health checks with 5-second intervals. The server
depends on PostgreSQL, NATS, and RustFS being healthy before it starts accepting
requests.

## Database Migrations

Migrations are embedded in the server binary and applied automatically on
startup. For manual control:

```bash
make generate-migrations   # Apply and regenerate schema
make clear-migrations      # Revert all (destructive)
```

## Commands

```bash
# Start services
docker compose up -d

# View logs
docker compose logs -f

# Stop services
docker compose down

# Reset data
docker compose down -v
```
