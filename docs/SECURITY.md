# Security

## Authentication

Nvisy supports multiple authentication methods:

**Password-based authentication** uses stateless JWT tokens signed with
Ed25519. Tokens carry the account identity, administrative status, and
standard RFC 7519 claims (issuer, audience, subject, issued-at, expiration,
token ID).

Token validation is multi-layered. The signature is verified against the
Ed25519 public key, the expiration is checked, and the token is confirmed
against the database to ensure the account still exists and is active. If the
administrative status recorded in the token disagrees with the current
database state, the token is rejected: this prevents privilege persistence
after role changes.

Passwords are hashed with Argon2id, the OWASP-recommended algorithm for
password storage. Each hash uses a unique cryptographically random salt.
Password strength is evaluated using zxcvbn, which considers dictionary
attacks, spatial keyboard patterns, repeated characters, and user-specific
inputs (email, name) to estimate crack time. Verification uses constant-time
comparison, and failed account lookups still execute a dummy hash to prevent
timing-based account enumeration.

**SSO integration** supports SAML 2.0 and OIDC for enterprise identity
providers. Organizations can enforce SSO-only authentication, disabling
password-based login for their workspace members. SSO sessions produce the
same JWT tokens used by password authentication, keeping downstream
authorization consistent.

**SCIM provisioning** automates user lifecycle management. Enterprise identity
providers can create, update, deactivate, and remove accounts through the
SCIM protocol, ensuring that workspace membership reflects the organization's
directory without manual intervention.

## Credential Encryption

Provider credentials (database connection strings, API keys, storage access
keys) are encrypted at rest using a two-tier key hierarchy.

A **master encryption key** is a 32-byte secret loaded from a file at server
startup. This key is never used directly for encryption. Instead, it derives a
unique **workspace key** for each workspace using HKDF-SHA256 (RFC 5869), with
the workspace ID as salt and a versioned domain separation string as context.
This design ensures that a compromise of one workspace's derived key cannot
expose credentials belonging to another workspace.

Each workspace key encrypts connection data using **XChaCha20-Poly1305**, an
authenticated encryption scheme. The 24-byte nonce is randomly generated per
encryption operation, which is safe for random generation given XChaCha20's
large nonce space. The ciphertext includes the Poly1305 authentication tag,
which prevents tampering: any modification to the ciphertext causes decryption
to fail.

Encrypted credentials are decrypted only when dispatching a job to the runtime.
They are never exposed through the API.

## Authorization

Access control uses a role-based model with four hierarchical workspace roles:

- **Guest:** Read-only access to workspace resources
- **Member:** Create and edit content, upload files, manage documents
- **Admin:** Manage members, connections, webhooks, and workspace settings
- **Owner:** Delete workspace, transfer ownership, manage all roles

Each API operation requires a specific permission, and each permission maps to a
minimum required role. Administrators bypass workspace-level permission checks.
File owners retain special privileges over their own resources regardless of
their workspace role.

Authorization is checked on every request after authentication. The
authenticated account's workspace membership and role are verified against the
required permission for the operation. Denial reasons are logged for audit
purposes.

## Rate Limiting

The server enforces per-client and per-tenant rate limits at the middleware
layer. Limits are configurable per endpoint category: authentication endpoints
have stricter limits to prevent brute-force attacks, file upload and job
dispatch endpoints have throughput-based limits, and read endpoints have
higher allowances.

Rate-limited responses return `429 Too Many Requests` with a `Retry-After`
header indicating when the client can retry. Rate limit state is tracked
per API token or session, and per workspace for tenant-scoped endpoints.

## HTTP Security

The server applies standard HTTP security headers on all responses:

- **HSTS:** Strict-Transport-Security with a one-year max-age and subdomain
  inclusion, directing browsers to use HTTPS exclusively
- **CSP:** Content-Security-Policy restricting scripts, styles, images, and
  frame ancestors to prevent XSS and clickjacking
- **X-Frame-Options:** Set to DENY, preventing the application from being
  embedded in frames
- **X-Content-Type-Options:** Set to nosniff, preventing MIME type sniffing
- **Referrer-Policy:** strict-origin-when-cross-origin, limiting referrer
  leakage

CORS is configurable per deployment, with explicit origin whitelisting,
credential support, and preflight caching.

Request body sizes are limited at the middleware layer (separate limits for
JSON payloads and file uploads) to prevent resource exhaustion.

## Input Validation

All API request bodies are validated before reaching handler logic. The
validation layer checks field constraints (length, format, range, pattern) and
returns structured error responses with field-specific messages. This prevents
malformed data from reaching the database or business logic.

## API Tokens

Accounts can create long-lived API tokens for programmatic access. Each token
is a signed JWT with the same claims as session tokens. Tokens can be listed,
updated (name, description), and revoked. Revocation is immediate: revoked
tokens are rejected on the next request.

Tokens support scoping to specific workspaces and permission sets. A scoped
token can only access resources within its designated workspaces and can only
perform operations allowed by its permission set, regardless of the account's
full permissions. This enforces the principle of least privilege for
programmatic integrations.

## Account Deletion

Accounts can be deleted through the API to satisfy GDPR right-to-erasure.
Account deletion cascades removal of all associated data: workspace
memberships, uploaded files, annotations, API tokens, notifications, and
activity records. A configurable grace period allows the account to be
recovered before permanent deletion. The deletion request itself is recorded
in the audit trail.

## Webhook Signing

Outbound webhook deliveries are signed with HMAC-SHA256. Each webhook has a
secret generated at creation time, shown once, and not retrievable afterward.
The signature covers both a Unix timestamp and the payload body, preventing
replay attacks. Recipients verify the signature by recomputing the HMAC with
their copy of the secret.

Webhook headers include the event type, a delivery timestamp, a unique request
ID (UUID v7 for traceability), and the signature.

## Audit Logging

The server maintains two complementary audit mechanisms:

**Structured tracing** logs security-relevant events in real time using the
tracing framework. Successful and failed authentication attempts,
authorization decisions, token validation outcomes, and password operations
are all recorded with account IDs, resource IDs, and failure reasons.
Sensitive data (passwords, tokens, encryption keys) is never included in log
output.

**Activity records** provide an append-only, tamper-evident audit trail
queryable through the API. Every mutating action (resource creation,
modification, deletion, role changes, policy updates) produces an immutable
activity record with the actor identity, action type, affected resource,
timestamp, and a summary of the change. Activity records are retained
according to configurable retention policies (typically longer than content
retention to meet regulatory requirements). Records cannot be modified or
deleted before their retention period expires.

The activity feed supports filtering by workspace, actor, action type, and
time range, enabling compliance reporting and forensic investigation.

For the runtime's compliance capabilities (policy engine, explainability,
and audit trail chain-of-custody), see the runtime's
[Compliance](https://github.com/nvisycom/runtime/blob/main/docs/COMPLIANCE.md)
documentation.
