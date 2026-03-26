# Providers

## Overview

The server manages connections to external systems: encrypted credential
storage, access control, and job dispatch. The actual reading from and writing
to external systems is handled by the
[Nvisy Runtime](https://github.com/nvisycom/runtime), which communicates with
the server over NATS.

For supported formats, extraction capabilities (OCR, speech-to-text, document
parsing), and output transformation, see the runtime's
[Ingestion](https://github.com/nvisycom/runtime/blob/main/docs/INGESTION.md)
documentation.

## What the Server Manages

### Connection Storage

Each connection record contains a provider identifier and an encrypted
credential blob. Encryption uses workspace-derived keys (HKDF-SHA256 +
XChaCha20-Poly1305) so that a compromise of one workspace cannot expose
another's credentials. Credentials are never exposed through the API.

### Credential Lifecycle

At processing time, the server decrypts connection credentials and passes them
to the runtime over NATS. The runtime uses these credentials to establish
sessions with external systems and retrieve or deliver documents. After the
job completes, the decrypted credentials exist only in the runtime's ephemeral
processing context.

### Document Upload

Documents can be uploaded directly to the server via multipart HTTP. The server
stores each document as an encrypted binary object in NATS object storage with
metadata (type, size, hash, version chain) tracked in PostgreSQL.

## Provider Abstraction

Every external system is accessed through a uniform provider interface
implemented in the runtime:

- **Connection:** Establishes a session using typed credentials decrypted by
  the server
- **Reading:** Retrieves documents with resumable pagination
- **Writing:** Sends redacted documents to the destination

### Adding a New Provider

Adding a provider does not require modifying the server. The process is:

1. Define the provider's connection parameters and credential schema
2. Implement the read and/or write interface in the runtime
3. Register the provider identifier so the runtime can dispatch to it

The server manages connections, credentials, and orchestration while the
runtime handles execution.
