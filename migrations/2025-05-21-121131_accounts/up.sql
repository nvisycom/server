-- Accounts: account identities and their API tokens. The foundational identity
-- tables every other resource references.

-- Kind of client an API token was issued to.
CREATE TYPE API_TOKEN_TYPE AS ENUM (
    'web',      -- Web browser session (cookie)
    'api',      -- API client session (programmatic access, user-minted)
    'app'       -- Native app session (interactive desktop login, Bearer token)
);

COMMENT ON TYPE API_TOKEN_TYPE IS 'Client kind an API token was issued to: web, api, or app.';

-- How an account authenticates. 'password' is a locally-held Argon2 secret; the
-- rest are external OIDC identity providers keyed by the provider's subject claim.
CREATE TYPE IDENTITY_PROVIDER AS ENUM (
    'password',     -- Local password (Argon2 hash)
    'google',       -- Google (OIDC)
    'microsoft'     -- Microsoft / Entra ID (OIDC)
);

COMMENT ON TYPE IDENTITY_PROVIDER IS 'Authentication method for an account identity: local password or an OIDC provider.';

-- Accounts table: one identity per person.
CREATE TABLE accounts (
    -- Primary identifier
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Status and permissions
    is_admin              BOOLEAN     NOT NULL DEFAULT FALSE,
    is_verified           BOOLEAN     NOT NULL DEFAULT FALSE,
    is_suspended          BOOLEAN     NOT NULL DEFAULT FALSE,

    -- Public account handle, unique across all accounts: lowercase alphanumeric
    -- with single internal dashes, 3-32 characters. Addresses the profile at
    -- /accounts/{username} and stands in for the account id at the API boundary.
    username              TEXT        NOT NULL,
    CONSTRAINT accounts_username_length CHECK (length(username) BETWEEN 3 AND 32),
    CONSTRAINT accounts_username_format CHECK (username ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),

    -- Core account information
    display_name          TEXT        DEFAULT NULL,
    email_address         TEXT        NOT NULL,
    CONSTRAINT accounts_display_name_length CHECK (display_name IS NULL OR length(trim(display_name)) BETWEEN 2 AND 32),
    CONSTRAINT accounts_display_name_not_empty CHECK (display_name IS NULL OR trim(display_name) <> ''),
    CONSTRAINT accounts_email_format CHECK (is_valid_email(email_address)),
    CONSTRAINT accounts_email_length_max CHECK (length(email_address) <= 254),

    -- Optional profile information
    avatar_url            TEXT        DEFAULT NULL,

    -- Preferences and settings
    timezone              TEXT        NOT NULL DEFAULT 'UTC',
    locale                TEXT        NOT NULL DEFAULT 'en-US',
    CONSTRAINT accounts_timezone_format CHECK (timezone ~ '^[A-Za-z_/]+$'),
    CONSTRAINT accounts_locale_format CHECK (locale ~ '^[a-z]{2}-[A-Z]{2}$'),

    -- Security tracking
    password_changed_at   TIMESTAMPTZ DEFAULT NULL,

    -- Lifecycle timestamps
    created_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    deleted_at            TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT accounts_updated_after_created CHECK (updated_at >= created_at),
    CONSTRAINT accounts_deleted_after_created CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT accounts_deleted_after_updated CHECK (deleted_at IS NULL OR deleted_at >= updated_at),
    CONSTRAINT accounts_password_changed_after_created CHECK (password_changed_at IS NULL OR password_changed_at >= created_at),

    -- An admin account cannot be suspended.
    CONSTRAINT accounts_suspended_not_admin CHECK (NOT (is_suspended AND is_admin))
);

-- Keep updated_at current on every write.
SELECT setup_updated_at('accounts');

-- Email and username are unique among live accounts, case-insensitively.
CREATE UNIQUE INDEX accounts_email_address_unique_idx
    ON accounts (lower(email_address))
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX accounts_username_unique_idx
    ON accounts (lower(username))
    WHERE deleted_at IS NULL;

-- Admins, for the admin listing.
CREATE INDEX accounts_admin_users_idx
    ON accounts (id, display_name)
    WHERE is_admin = TRUE AND deleted_at IS NULL;

-- Fuzzy name/email search over live accounts.
CREATE INDEX accounts_display_name_trgm_idx
    ON accounts USING gin (display_name gin_trgm_ops)
    WHERE deleted_at IS NULL;

CREATE INDEX accounts_email_address_trgm_idx
    ON accounts USING gin (email_address gin_trgm_ops)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE accounts IS 'Account identities, with preferences and security tracking.';
COMMENT ON COLUMN accounts.id IS 'Unique account identifier';
COMMENT ON COLUMN accounts.is_admin IS 'Administrative privileges across the whole system';
COMMENT ON COLUMN accounts.is_verified IS 'Whether the account has confirmed its email';
COMMENT ON COLUMN accounts.is_suspended IS 'Whether account access is temporarily disabled';
COMMENT ON COLUMN accounts.username IS 'Public handle, unique across accounts (3-32 chars, lowercase, dash-separated)';
COMMENT ON COLUMN accounts.display_name IS 'Optional human-readable name for display (2-32 chars)';
COMMENT ON COLUMN accounts.email_address IS 'Primary email for sign-in and contact';
COMMENT ON COLUMN accounts.avatar_url IS 'URL of the profile image';
COMMENT ON COLUMN accounts.timezone IS 'Preferred timezone for date/time display';
COMMENT ON COLUMN accounts.locale IS 'Preferred locale for language and formatting';
COMMENT ON COLUMN accounts.password_changed_at IS 'When the password was last changed; NULL if never';
COMMENT ON COLUMN accounts.created_at IS 'Account creation timestamp';
COMMENT ON COLUMN accounts.updated_at IS 'Last-modified timestamp (kept current by trigger)';
COMMENT ON COLUMN accounts.deleted_at IS 'Soft-deletion timestamp; NULL means live';

-- Account identities: one row per way an account can authenticate. An account
-- has at least one (a password or a linked provider) and may have several (a
-- password plus one or more OIDC providers). Credentials live here, never on the
-- account, so authentication methods are decoupled from identity.
CREATE TABLE account_identities (
    -- Primary identifier
    id                    UUID           PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id            UUID           NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Which authentication method this identity is.
    provider              IDENTITY_PROVIDER NOT NULL,

    -- For a local password: the Argon2 hash. NULL for an OIDC identity.
    secret                TEXT           DEFAULT NULL,

    -- For an OIDC identity: the provider's stable subject (`sub`) claim, not the
    -- email (an address can be reassigned within a directory, `sub` cannot). Each
    -- provider is pinned to a single issuer (Google's, or one Microsoft tenant),
    -- so `(provider, provider_subject)` is unique on its own. NULL for a password
    -- identity.
    provider_subject      TEXT           DEFAULT NULL,

    -- The email the provider asserted at link time (OIDC), or NULL for a password
    -- identity. Informational; `accounts.email_address` remains the account's
    -- primary address.
    provider_email        TEXT           DEFAULT NULL,

    -- Lifecycle timestamps
    created_at            TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,
    updated_at            TIMESTAMPTZ    NOT NULL DEFAULT current_timestamp,

    -- A password identity carries a secret and no subject; an OIDC identity
    -- carries a subject and no secret. The provider discriminates the two shapes.
    CONSTRAINT account_identities_password_shape CHECK (
        provider <> 'password'
        OR (
            secret IS NOT NULL AND length(secret) >= 60
            AND provider_subject IS NULL
        )
    ),
    CONSTRAINT account_identities_oidc_shape CHECK (
        provider = 'password'
        OR (
            provider_subject IS NOT NULL AND provider_subject <> ''
            AND secret IS NULL
        )
    ),
    CONSTRAINT account_identities_updated_after_created CHECK (updated_at >= created_at)
);

-- Keep updated_at current on every write.
SELECT setup_updated_at('account_identities');

-- An account has at most one identity per provider (one password, one Google, …).
CREATE UNIQUE INDEX account_identities_account_provider_unique_idx
    ON account_identities (account_id, provider);

-- A (provider, subject) pair maps to exactly one identity, so a returning OIDC
-- user is resolved by it. Each provider is pinned to a single issuer, so the
-- subject is unique within the provider. Password identities carry no subject and
-- are excluded.
CREATE UNIQUE INDEX account_identities_provider_subject_unique_idx
    ON account_identities (provider, provider_subject)
    WHERE provider_subject IS NOT NULL;

-- All identities for an account, for the login lookup and account settings.
CREATE INDEX account_identities_account_idx
    ON account_identities (account_id);

COMMENT ON TABLE account_identities IS 'Authentication methods for an account: a local password and/or linked OIDC providers.';
COMMENT ON COLUMN account_identities.id IS 'Unique identity identifier';
COMMENT ON COLUMN account_identities.account_id IS 'Account this identity authenticates';
COMMENT ON COLUMN account_identities.provider IS 'Authentication method: password or an OIDC provider';
COMMENT ON COLUMN account_identities.secret IS 'Argon2 password hash for a password identity; NULL for OIDC';
COMMENT ON COLUMN account_identities.provider_subject IS 'OIDC provider subject (sub) claim; NULL for a password identity';
COMMENT ON COLUMN account_identities.provider_email IS 'Email the provider asserted at link time; NULL for a password identity';
COMMENT ON COLUMN account_identities.created_at IS 'Identity creation timestamp';
COMMENT ON COLUMN account_identities.updated_at IS 'Last-modified timestamp (kept current by trigger)';

-- API tokens table: one row per issued authentication token.
CREATE TABLE account_api_tokens (
    -- Primary identifier
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- References
    account_id            UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,

    -- Token metadata
    display_name          TEXT        NOT NULL,
    session_type          API_TOKEN_TYPE NOT NULL DEFAULT 'web',
    CONSTRAINT account_api_tokens_display_name_not_empty CHECK (trim(display_name) <> ''),
    CONSTRAINT account_api_tokens_display_name_length CHECK (length(display_name) <= 100),

    -- Security context
    ip_address            INET        DEFAULT NULL,
    user_agent            TEXT        DEFAULT NULL,
    is_remembered         BOOLEAN     NOT NULL DEFAULT FALSE,

    -- Lifecycle timestamps
    issued_at             TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    expired_at            TIMESTAMPTZ DEFAULT NULL,
    last_used_at          TIMESTAMPTZ DEFAULT NULL,
    deleted_at            TIMESTAMPTZ DEFAULT NULL,
    CONSTRAINT account_api_tokens_expired_after_issued CHECK (expired_at IS NULL OR expired_at > issued_at),
    CONSTRAINT account_api_tokens_deleted_after_issued CHECK (deleted_at IS NULL OR deleted_at >= issued_at),
    CONSTRAINT account_api_tokens_last_used_after_issued CHECK (last_used_at IS NULL OR last_used_at >= issued_at)
);

-- Active (non-expired, live) tokens for an account.
CREATE INDEX account_api_tokens_account_active_idx
    ON account_api_tokens (account_id, expired_at)
    WHERE deleted_at IS NULL;

-- Expiry sweep for the cleanup job.
CREATE INDEX account_api_tokens_cleanup_idx
    ON account_api_tokens (expired_at)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE account_api_tokens IS 'Authentication tokens issued to an account.';
COMMENT ON COLUMN account_api_tokens.id IS 'Unique token identifier';
COMMENT ON COLUMN account_api_tokens.account_id IS 'Account this token belongs to';
COMMENT ON COLUMN account_api_tokens.display_name IS 'Human-readable token name (max 100 chars)';
COMMENT ON COLUMN account_api_tokens.session_type IS 'Client kind that created the session (web, api, cli)';
COMMENT ON COLUMN account_api_tokens.ip_address IS 'IP address the session was created from';
COMMENT ON COLUMN account_api_tokens.user_agent IS 'User agent of the creating client';
COMMENT ON COLUMN account_api_tokens.is_remembered IS 'Whether the token uses extended expiration (remember me)';
COMMENT ON COLUMN account_api_tokens.issued_at IS 'Token creation timestamp';
COMMENT ON COLUMN account_api_tokens.expired_at IS 'Expiration timestamp; NULL never expires';
COMMENT ON COLUMN account_api_tokens.last_used_at IS 'Most recent use timestamp';
COMMENT ON COLUMN account_api_tokens.deleted_at IS 'Revocation timestamp; NULL means live';
