# Nvisy Server

## Overview

Nvisy Server is the API gateway for the Nvisy multimodal redaction platform. It
handles authentication, authorization, workspace management, credential
encryption, document storage, and API serving. The actual detection and
redaction of sensitive data is performed by the
[Nvisy Runtime](https://github.com/nvisycom/runtime), which communicates with
the server over NATS.

Together, the server and runtime form a platform that detects and removes
personally identifiable information (PII), protected health information (PHI),
and other sensitive data across documents, images, and audio using AI-powered
detection with configurable, policy-driven redaction.

The guiding principle is: **extract everything, understand context, redact
precisely, prove compliance.**

## What the Server Does

The server is responsible for everything outside the detection and redaction
pipeline itself:

- **Authentication and authorization:** JWT-based auth with Ed25519 signing,
  SSO (SAML/OIDC), SCIM provisioning, role-based access control per workspace,
  scoped API token management
- **Account management:** User profiles, account deletion (GDPR),
  notifications, and immutable activity feeds
- **Workspace management:** Multi-tenant workspace hierarchy with
  cryptographic isolation, member and invite management, role-based permissions
- **Credential management:** Encrypted storage of provider credentials using
  workspace-derived keys (HKDF-SHA256 + XChaCha20-Poly1305)
- **Document storage:** File upload, versioning, annotations, and metadata
  tracking via NATS object storage and PostgreSQL
- **Pipeline orchestration:** Processing workflow definitions, scheduling,
  run history, and artifact tracking
- **Webhooks:** Event subscription management with HMAC-SHA256 signed
  delivery and retry support
- **Data retention:** Configurable retention policies with scheduled cleanup,
  zero-retention mode for sensitive environments
- **API gateway:** Versioned REST API with OpenAPI docs, request validation,
  rate limiting, idempotency keys, and cursor-based pagination
- **Real-time collaboration:** WebSocket and NATS pub/sub for Studio sessions
- **Observability:** Per-component health checks, structured logging, and
  distributed tracing

The runtime handles detection, redaction, format processing, and policy
evaluation. The server orchestrates these operations, manages the data they
operate on, and enforces the security boundary around them.

## Target Verticals

The platform serves regulated industries where sensitive data handling is a
legal and operational requirement:

- **Healthcare:** HIPAA-governed medical records, clinical communications,
  insurance claims, and patient intake forms
- **Legal:** Court filings, discovery documents, attorney-client
  communications, and case management systems
- **Government and defense:** Law enforcement records, intelligence reports,
  FOIA responses, and classified material processing
- **Financial services:** Transaction records, customer onboarding documents,
  fraud investigation files, and PCI-scoped payment data
- **Education:** Student records, admissions documents, and FERPA-governed
  institutional data

## Documentation

| Document                          | Description                                                           |
| --------------------------------- | --------------------------------------------------------------------- |
| [Architecture](./ARCHITECTURE.md) | System design, deployment models, and technology stack                |
| [Intelligence](./INTELLIGENCE.md) | Policy management, job dispatch, and runtime interaction              |
| [Providers](./PROVIDERS.md)       | Connection management, credential encryption, and document upload     |
| [Security](./SECURITY.md)         | Authentication, encryption, authorization, and audit logging          |

## Deployment

Infrastructure requirements, configuration reference, and Docker setup are
documented in the [`docker/`](../docker/) directory.
