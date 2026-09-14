//! Ed25519 signing keys for JWT sessions.
//!
//! [`AuthKeys`] holds the encoding (private) and decoding (public) keys used
//! to sign and verify session tokens. Build it from PEM material directly with
//! [`AuthKeys::from_pem`], or from the two key files a deployment configures
//! with [`AuthKeysConfig::load`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey};

use crate::{Error, Result};

/// Tracing target for session key operations.
const TRACING_TARGET: &str = "nvisy_server::service::auth_keys";

/// The file paths a deployment loads its session keys from.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct AuthKeysConfig {
    /// PEM file holding the JWT decoding (public) key.
    #[cfg_attr(
        feature = "cli",
        arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH", default_value = "./public.pem")
    )]
    pub decoding_key: PathBuf,

    /// PEM file holding the JWT encoding (private) key.
    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            env = "AUTH_PRIVATE_PEM_FILEPATH",
            default_value = "./private.pem"
        )
    )]
    pub encoding_key: PathBuf,
}

impl Default for AuthKeysConfig {
    fn default() -> Self {
        Self {
            decoding_key: "./public.pem".into(),
            encoding_key: "./private.pem".into(),
        }
    }
}

/// The Ed25519 keypair used to sign and verify session tokens. Cheap to clone
/// (the keys sit behind an `Arc`).
#[derive(Clone)]
pub struct AuthKeys {
    inner: Arc<Keys>,
}

/// The decoding/encoding keys, shared behind [`AuthKeys`]'s `Arc`.
struct Keys {
    decoding_key: DecodingKey,
    encoding_key: EncodingKey,
}

impl AuthKeys {
    /// Builds the keys from PEM material — the real constructor, with no
    /// filesystem dependency. [`AuthKeysConfig::load`] reads its files and
    /// calls this.
    ///
    /// # Errors
    ///
    /// An auth error if either PEM does not parse as an Ed25519 key.
    pub fn from_pem(decoding_pem: &[u8], encoding_pem: &[u8]) -> Result<Self> {
        let decoding_key = DecodingKey::from_ed_pem(decoding_pem).map_err(|e| {
            tracing::error!(target: TRACING_TARGET, error = %e, "failed to parse decoding key PEM");
            Error::auth("invalid decoding key PEM format").with_source(e)
        })?;
        let encoding_key = EncodingKey::from_ed_pem(encoding_pem).map_err(|e| {
            tracing::error!(target: TRACING_TARGET, error = %e, "failed to parse encoding key PEM");
            Error::auth("invalid encoding key PEM format").with_source(e)
        })?;

        Ok(Self {
            inner: Arc::new(Keys {
                decoding_key,
                encoding_key,
            }),
        })
    }

    /// The decoding key, used to verify session tokens.
    #[inline]
    #[must_use]
    pub fn decoding_key(&self) -> &DecodingKey {
        &self.inner.decoding_key
    }

    /// The encoding key, used to sign session tokens.
    #[inline]
    #[must_use]
    pub fn encoding_key(&self) -> &EncodingKey {
        &self.inner.encoding_key
    }
}

impl AuthKeysConfig {
    /// Reads the two key files and builds the [`AuthKeys`] — the one place the
    /// session keys touch the filesystem; [`AuthKeys::from_pem`] parses them.
    ///
    /// # Errors
    ///
    /// - A config error if either key file is missing or is not a regular file.
    /// - A file-system error if reading a key file fails.
    /// - An auth error if a key file's PEM does not parse as an Ed25519 key.
    pub async fn load(&self) -> Result<AuthKeys> {
        let decoding_pem = read_key_file(&self.decoding_key, "decoding").await?;
        let encoding_pem = read_key_file(&self.encoding_key, "encoding").await?;

        tracing::info!(target: TRACING_TARGET, "Session keys loaded");
        AuthKeys::from_pem(&decoding_pem, &encoding_pem)
    }
}

/// Reads a key file after checking it exists and is a regular file. `which`
/// names the key ("decoding" / "encoding") in errors and logs.
async fn read_key_file(path: &Path, which: &str) -> Result<Vec<u8>> {
    if !path.exists() {
        return Err(Error::config(format!("{which} key file does not exist")));
    }
    if !path.is_file() {
        return Err(Error::config(format!("{which} key path is not a file")));
    }
    tokio::fs::read(path).await.map_err(|e| {
        tracing::error!(target: TRACING_TARGET, key = which, path = %path.display(), error = %e, "failed to read key file");
        Error::file_system(format!("failed to read {which} key file")).with_source(e)
    })
}

impl fmt::Debug for AuthKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthKeys")
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const TEST_PRIVATE_KEY: &str = r"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIDQtFc/jcCECuwR6cQqh9Xy3y8pcryWDn/HVN5fPSwm+
-----END PRIVATE KEY-----";

    const TEST_PUBLIC_KEY: &str = r"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAMveirBCUUpVI8TCv4W5jAZqtkEzfA7eIvozsugFbvDU=
-----END PUBLIC KEY-----";

    /// A config pointing at two files written into `dir`.
    fn write_keys(dir: &TempDir, public: &str, private: &str) -> AuthKeysConfig {
        let decoding_key = dir.path().join("public.pem");
        let encoding_key = dir.path().join("private.pem");
        fs::write(&decoding_key, public).unwrap();
        fs::write(&encoding_key, private).unwrap();
        AuthKeysConfig {
            decoding_key,
            encoding_key,
        }
    }

    #[test]
    fn from_pem_builds_a_valid_keypair() {
        // A signed token must verify with the paired decoding key.
        let keys =
            AuthKeys::from_pem(TEST_PUBLIC_KEY.as_bytes(), TEST_PRIVATE_KEY.as_bytes()).unwrap();
        assert!(round_trips(&keys));
    }

    #[tokio::test]
    async fn load_reads_the_key_files() {
        let dir = TempDir::new().unwrap();
        let keys = write_keys(&dir, TEST_PUBLIC_KEY, TEST_PRIVATE_KEY)
            .load()
            .await
            .unwrap();
        assert!(round_trips(&keys));
    }

    #[tokio::test]
    async fn load_rejects_an_unparseable_key() {
        let dir = TempDir::new().unwrap();
        assert!(
            write_keys(&dir, "not a pem", TEST_PRIVATE_KEY)
                .load()
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn load_rejects_missing_files() {
        let dir = TempDir::new().unwrap();
        let config = AuthKeysConfig {
            decoding_key: dir.path().join("missing_public.pem"),
            encoding_key: dir.path().join("missing_private.pem"),
        };
        assert!(config.load().await.is_err());
    }

    /// Whether a token signed with the encoding key verifies with the decoding
    /// key — proves the pair matches.
    fn round_trips(keys: &AuthKeys) -> bool {
        use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct Claims {
            sub: String,
            exp: usize,
        }

        let claims = Claims {
            sub: "test".to_owned(),
            exp: usize::MAX,
        };
        let Ok(token) = encode(&Header::new(Algorithm::EdDSA), &claims, keys.encoding_key()) else {
            return false;
        };
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = false;
        decode::<Claims>(&token, keys.decoding_key(), &validation).is_ok()
    }
}
