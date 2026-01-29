# Security

## Authentication

Nvisy uses stateless JWT authentication with Ed25519 signing. Tokens carry the
account identity, administrative status, and standard RFC 7519 claims (issuer,
audience, subject, issued-at, expiration, token ID).

Token validation is multi-layered. The signature is verified against the Ed25519
public key, the expiration is checked, and the token is confirmed against the
database to ensure the account still exists and is active. If the administrative
status recorded in the token disagrees with the current database state, the
token is rejected — this prevents privilege persistence after role changes.

Passwords are hashed with Argon2id, the OWASP-recommended algorithm for password
storage. Each hash uses a unique cryptographically random salt. Password
strength is evaluated using zxcvbn, which considers dictionary attacks, spatial
keyboard patterns, repeated characters, and user-specific inputs (email, name)
to estimate crack time. Verification uses constant-time comparison, and failed
account lookups still execute a dummy hash to prevent timing-based account
enumeration.

## Credential Encryption

Provider credentials — database connection strings, API keys, storage access
keys — are encrypted at rest using a two-tier key hierarchy.

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
which prevents tampering — any modification to the ciphertext causes decryption
to fail.

Encrypted credentials are decrypted only at pipeline execution time, within the
scope of a single run. They are never exposed through the API.

## Authorization

Access control uses a role-based model with four hierarchical workspace roles:

- **Guest** — read-only access to workspace resources
- **Member** — create and edit content, upload files, manage pipelines
- **Admin** — manage members, connections, webhooks, and workspace settings
- **Owner** — delete workspace, transfer ownership, manage all roles

Each API operation requires a specific permission, and each permission maps to a
minimum required role. Administrators bypass workspace-level permission checks.
File owners retain special privileges over their own resources regardless of
their workspace role.

Authorization is checked on every request after authentication. The
authenticated account's workspace membership and role are verified against the
required permission for the operation. Denial reasons are logged for audit
purposes.

## HTTP Security

The server applies standard HTTP security headers on all responses:

- **HSTS** — Strict-Transport-Security with a one-year max-age and subdomain
  inclusion, directing browsers to use HTTPS exclusively
- **CSP** — Content-Security-Policy restricting scripts, styles, images, and
  frame ancestors to prevent XSS and clickjacking
- **X-Frame-Options** — set to DENY, preventing the application from being
  embedded in frames
- **X-Content-Type-Options** — set to nosniff, preventing MIME type sniffing
- **Referrer-Policy** — strict-origin-when-cross-origin, limiting referrer
  leakage

CORS is configurable per deployment, with explicit origin whitelisting,
credential support, and preflight caching.

Request body sizes are limited at the middleware layer — separate limits for
JSON payloads and file uploads — to prevent resource exhaustion.

## Input Validation

All API request bodies are validated before reaching handler logic. The
validation layer checks field constraints — length, format, range, pattern — and
returns structured error responses with field-specific messages. This prevents
malformed data from reaching the database or business logic.

## Webhook Signing

Outbound webhook deliveries are signed with HMAC-SHA256. Each webhook has a
secret generated at creation time, shown once, and not retrievable afterward.
The signature covers both a Unix timestamp and the payload body, preventing
replay attacks. Recipients verify the signature by recomputing the HMAC with
their copy of the secret.

Webhook headers include the event type, a delivery timestamp, a unique request
ID (UUID v7 for traceability), and the signature.

## Audit Logging

Security-relevant events are logged with structured fields using the tracing
framework. Successful and failed authentication attempts, authorization
decisions, token validation outcomes, and password operations are all recorded
with account IDs, resource IDs, and failure reasons. Sensitive data — passwords,
tokens, encryption keys — is never included in log output.
