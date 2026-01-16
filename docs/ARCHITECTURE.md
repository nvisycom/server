# Architecture

## Crate Structure

| Crate | Responsibility |
|-------|----------------|
| `nvisy-server` | HTTP API, handlers, middleware, auth |
| `nvisy-postgres` | Database models, queries, migrations |
| `nvisy-nats` | Messaging, job queues, object storage |
| `nvisy-rig` | LLM orchestration, RAG, chat agents |
| `nvisy-webhook` | External event delivery |
| `nvisy-core` | Shared types and utilities |
| `nvisy-cli` | Command-line interface |

## Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Language | Rust | Memory safety, performance, concurrency |
| Database | PostgreSQL + pgvector | Relational data + vector embeddings |
| Messaging | NATS | Pub/sub, job queues, object storage |
| AI Framework | rig-core | LLM orchestration |
| HTTP Server | Axum + Tower | API endpoints and middleware |
| Real-time | SSE | Streaming AI responses |
| Auth | JWT | Stateless authentication |

## Data Model

### Core Entities

| Entity | Purpose |
|--------|---------|
| Account | User authentication and profile |
| Workspace | Collaborative space for documents |
| Document | Logical grouping of related files |
| File | Individual uploaded file with metadata |
| Version | Parsed representation at a point in time |
| Section | Hierarchical content structure |
| Chunk | Indexed segment with vector embedding |
| Entity | Extracted person, company, date, amount |
| ChatSession | AI conversation context |

### Hierarchy

- **Workspace** contains Documents
- **Document** contains Files and Versions
- **Version** contains Sections
- **Section** contains Chunks
- **Chunk** contains Entities, Claims, and References

### Content Types

| Type | Examples | Processing |
|------|----------|------------|
| Document | PDF, DOCX, TXT | Text extraction, structure parsing |
| Image | PNG, JPG, SVG | OCR, visual analysis |
| Spreadsheet | XLSX, CSV | Table normalization, schema inference |
| Presentation | PPTX, KEY | Slide extraction, structure parsing |
| Audio | MP3, WAV | Transcription with timestamps |
| Video | MP4, MOV | Transcription, frame extraction |
| Archive | ZIP, TAR | Recursive extraction and processing |
| Data | JSON, XML | Schema inference, normalization |

## Canonical Representation

All source files normalize to a common schema containing:

- **Sections**: Hierarchical structure
- **Entities**: People, companies, dates, amounts
- **Tables**: Structured data
- **Claims**: Assertions that can be verified
- **References**: Links to other documents/sections
- **Provenance**: Source file, extraction method, confidence

## Chunking Strategy

Effective cross-file intelligence depends on chunking quality.

Requirements:
- **Semantic chunks**: Based on meaning, not fixed token sizes
- **Stable chunk IDs**: Enable diffs, history, and references
- **Hierarchical chunks**: Document → Section → Paragraph → Sentence

Each chunk maintains: stable content-addressable ID, hierarchical location, vector embedding, extracted entities, token count, and byte range in source.
