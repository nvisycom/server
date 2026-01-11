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

## Optional Integrations (Development)

The development compose file includes optional services that can be enabled using Docker Compose profiles. These are useful for testing integrations locally.

### Available Profiles

| Profile        | Services       | Description                          |
| -------------- | -------------- | ------------------------------------ |
| `minio`        | MinIO          | S3-compatible object storage         |
| `n8n`          | N8n            | Workflow automation platform         |
| `integrations` | MinIO + N8n    | All optional integration services    |

### Optional Services

| Service | Port(s)     | Console URL             | Description                  |
| ------- | ----------- | ----------------------- | ---------------------------- |
| MinIO   | 9000, 9001  | http://localhost:9001   | S3-compatible object storage |
| N8n     | 5678        | http://localhost:5678   | Workflow automation          |

### Usage

Start core services only (PostgreSQL + NATS):

```bash
docker compose -f docker-compose.dev.yml up -d
```

Start with MinIO:

```bash
docker compose -f docker-compose.dev.yml --profile minio up -d
```

Start with N8n:

```bash
docker compose -f docker-compose.dev.yml --profile n8n up -d
```

Start with all integrations:

```bash
docker compose -f docker-compose.dev.yml --profile integrations up -d
```

### Default Credentials

| Service | Username     | Password     | Environment Variables                            |
| ------- | ------------ | ------------ | ------------------------------------------------ |
| MinIO   | `minioadmin` | `minioadmin` | `MINIO_ROOT_USER`, `MINIO_ROOT_PASSWORD`         |
| N8n     | `admin`      | `admin`      | `N8N_BASIC_AUTH_USER`, `N8N_BASIC_AUTH_PASSWORD` |

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
