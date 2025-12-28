# Docker

Docker configuration for Nvisy API Server.

## Quick Start

### Development (infrastructure only)

Start PostgreSQL (with pgvector) and NATS for local development:

```bash
cd docker
cp .env.example .env
docker compose -f docker-compose.dev.yml up -d
```

Then run the server locally:

```bash
cargo run --release --bin nvisy-server
```

### Production

Build and run the complete stack:

```bash
cd docker
cp .env.example .env
# Edit .env with production values
docker compose up -d --build
```

## Services

| Service    | Port(s)     | Description                      |
| ---------- | ----------- | -------------------------------- |
| PostgreSQL | 5432        | Primary database (with pgvector) |
| NATS       | 4222, 8222  | Message queue (JetStream)        |
| Server     | 8080        | Nvisy API                        |

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
