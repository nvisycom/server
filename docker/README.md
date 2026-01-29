# Docker

Docker configuration for the Nvisy server.

## Infrastructure Requirements

Nvisy requires two external services:

**PostgreSQL 18+** with the pgvector extension. PostgreSQL serves as the primary
data store for all application state — accounts, workspaces, pipelines,
connections, file metadata — and provides vector similarity search through
pgvector. The recommended image is `pgvector/pgvector:pg18`.

**NATS 2.10+** with JetStream enabled. NATS handles three concerns: pub/sub
messaging for real-time events, persistent job queues for asynchronous
processing, and object storage for uploaded files. JetStream must be enabled
with sufficient storage allocation — the default configuration uses 1 GB of
memory store and 10 GB of file store.

## Quick Start

### Development (infrastructure only)

Start PostgreSQL (with pgvector) and NATS for local development:

```bash
docker compose -f docker-compose.dev.yml up -d
```

This starts both services with development defaults (`postgres:postgres`
credentials, JetStream enabled). Then generate configuration and run the server
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

The production compose file starts all three services on a private bridge
network. The server waits for PostgreSQL and NATS health checks to pass before
starting.

## Services

| Service    | Port(s)    | Description                      |
| ---------- | ---------- | -------------------------------- |
| PostgreSQL | 5432       | Primary database (with pgvector) |
| NATS       | 4222, 8222 | Message queue (JetStream)        |
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

1. **Planner** — generates a dependency recipe with cargo-chef
2. **Builder** — builds dependencies from the recipe (cached), then builds the
   server binary and strips it
3. **Runtime** — minimal Debian image with only the binary and runtime libraries

The runtime image runs as a non-root user (`nvisy`, UID 1000) and includes a
health check endpoint at `/health`.

## NATS Configuration

The default NATS configuration (`nats.conf`) enables JetStream with:

- 1 GB memory store for high-throughput streams
- 10 GB file store for persistent data
- 8 MB maximum payload size

Adjust these values based on expected workload. The memory store is used for
ephemeral streams; the file store is used for durable subscriptions and object
storage.

## Health Checks

All services expose health check endpoints:

| Service    | Endpoint                | Method   |
| ---------- | ----------------------- | -------- |
| Server     | `/health`               | HTTP GET |
| PostgreSQL | `pg_isready`            | CLI      |
| NATS       | `/healthz` on port 8222 | HTTP GET |

The compose files configure health checks with 5-second intervals. The server
depends on both PostgreSQL and NATS being healthy before it starts accepting
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
