//! OneDrive provider, over the Microsoft Graph API and a bearer access token.
//!
//! Files are addressed by their stable Graph `id`. The download endpoint answers
//! with a `302` to a short-lived, pre-authenticated URL; reqwest follows it (and
//! drops the bearer header on the cross-host hop, as the target requires). The
//! `common` tenant supports both personal and work/school accounts.

use reqwest::header::CONTENT_LENGTH;
use serde::Deserialize;

use super::{FileServiceConfig, ProviderRequest, encode_path_segment, response_stream};
use crate::client::{ByteStream, FileService, FileServiceClient, FileUpload, PickerAccessToken};
use crate::error::{Error, ErrorKind, Result};
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
///
/// Must match `FileServiceProvider::OneDrive`'s serde tag (`one_drive`) so the
/// stored provider column and the serialized config agree.
pub const PROVIDER_ID: &str = "one_drive";

/// Microsoft identity platform authorize endpoint (multi-tenant + personal).
const AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
/// Microsoft identity platform token endpoint.
const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
/// Microsoft Graph v1.0 base.
const API_BASE: &str = "https://graph.microsoft.com/v1.0";

/// The OAuth endpoints and scopes for OneDrive (Microsoft Graph).
#[must_use]
pub fn oauth_provider() -> OAuthProvider {
    OAuthProvider {
        auth_url: AUTH_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        // Files.ReadWrite covers download and upload; offline_access yields the
        // refresh token; User.Read enables the cheap /me verify.
        scopes: vec![
            "Files.ReadWrite".to_owned(),
            "offline_access".to_owned(),
            "User.Read".to_owned(),
        ],
        extra_authorize_params: Vec::new(),
    }
}

/// Mints a v8 file-picker token for a OneDrive connection, scoped to the
/// SharePoint resource that backs the account's drive.
///
/// A OneDrive for Business drive is a SharePoint personal site, and the picker
/// requires a SharePoint-audience token (not the Graph token the connector uses).
/// This resolves that per-account SharePoint host from Graph via `service`, then
/// mints a `{resource}/.default` token from the stored refresh token — a scope
/// subset of the existing grant, so no re-consent. `resource`, when given, is the
/// exact resource the picker named in its `authenticate` command; otherwise the
/// resolved host is used. The resolved host also gates account type. The
/// connector's own Graph token is never disturbed.
///
/// The OneDrive arm of [`FileServiceProvider::mint_picker_token`](super::FileServiceProvider::mint_picker_token),
/// kept here so the dispatch holds no provider-specific logic.
///
/// # Errors
///
/// Returns [`ErrorKind::BadRequest`](crate::error::ErrorKind::BadRequest) for a
/// personal/consumer account (no SharePoint host) — the modern picker is only
/// supported for OneDrive for Business — or an auth error if the token cannot be
/// minted.
pub(super) async fn mint_picker_token(
    service: &FileService,
    config: &FileServiceConfig,
    resource: Option<&str>,
) -> Result<PickerAccessToken> {
    // The Graph token is needed to resolve the SharePoint host; a refresh here is
    // handed back so the caller can persist a rotated refresh token.
    let fresh = service.ensure_fresh(config).await?;
    let effective = fresh.refreshed.as_ref().unwrap_or(config);

    let host = resolve_sharepoint_host(service.http(), &fresh.access_token)
        .await?
        .ok_or_else(|| {
            Error::new(
                ErrorKind::BadRequest,
                "the OneDrive file picker is available only for work or school \
                 (OneDrive for Business) accounts, not personal accounts",
            )
        })?;

    // The picker names the resource it wants per `authenticate` command; fall back
    // to the account's SharePoint host. A client-supplied resource is untrusted —
    // it selects the audience the minted `.default` token is scoped to — so accept
    // it only when it belongs to the same SharePoint tenant as the resolved host.
    // Otherwise the caller could mint a token scoped to an arbitrary resource.
    let resource = match resource {
        Some(requested) if same_sharepoint_tenant(&host, requested) => requested,
        Some(_) => {
            return Err(Error::new(
                ErrorKind::BadRequest,
                "the requested picker resource is not in this account's SharePoint tenant",
            ));
        }
        None => &host,
    };
    let scope = format!("{}/.default", resource.trim_end_matches('/'));

    let tokens = service
        .mint_scoped_token(effective, std::slice::from_ref(&scope))
        .await?;

    Ok(PickerAccessToken {
        access_token: tokens.access_token,
        expires_at: tokens.expires_at,
        refreshed: fresh.refreshed,
    })
}

/// Resolves the SharePoint host that backs a OneDrive account's drive, using a
/// Microsoft Graph access token. Returns `None` for a consumer/personal account,
/// whose drive is on the legacy consumer OneDrive service and has no SharePoint
/// host.
///
/// A OneDrive for Business drive is a SharePoint personal site, so `webUrl` is a
/// `https://{tenant}-my.sharepoint.com/...` URL; the origin of that URL is the
/// audience the v8 file picker's tokens must target. A personal account's `webUrl`
/// points at `onedrive.live.com` (no SharePoint), which is why the modern picker
/// is unsupported there.
///
/// # Errors
///
/// Returns an error if the Graph request fails or its body cannot be parsed.
pub(super) async fn resolve_sharepoint_host(
    http: &reqwest::Client,
    graph_access_token: &str,
) -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct DriveResponse {
        #[serde(rename = "webUrl")]
        web_url: Option<String>,
    }

    let response = http
        .get(format!("{API_BASE}/me/drive?$select=webUrl"))
        .bearer_auth(graph_access_token)
        .send()
        .await
        .map_err(|err| Error::connection("failed to query OneDrive drive").with_source(err))?
        // Classify by status via the crate's `From<reqwest::Error>`, so 401 maps
        // to an auth error while a 429 throttle or 5xx outage maps to a transient
        // kind rather than an auth failure.
        .error_for_status()
        .map_err(Error::from)?
        .json::<DriveResponse>()
        .await
        .map_err(|err| Error::connection("invalid OneDrive drive response").with_source(err))?;

    let host = response.web_url.and_then(|url| {
        let parsed = reqwest::Url::parse(&url).ok()?;
        let host = parsed.host_str()?;
        // A business (OneDrive for Business) drive lives on `*.sharepoint.com`; a
        // personal drive does not, and yields `None` here.
        host.to_ascii_lowercase()
            .ends_with(".sharepoint.com")
            .then(|| format!("https://{host}"))
    });

    Ok(host)
}

/// Whether a client-supplied picker `resource` belongs to the same SharePoint
/// tenant as the account's resolved `host`.
///
/// The picker may legitimately name either the tenant's SharePoint root
/// (`{tenant}.sharepoint.com`) or its personal-site host
/// (`{tenant}-my.sharepoint.com`), so the check is on the tenant label rather
/// than an exact host match: both hosts must be `*.sharepoint.com` and share the
/// tenant label once a `-my` suffix is dropped. Anything else — a different
/// tenant, or a non-SharePoint host — is rejected, so a minted token cannot be
/// scoped to an arbitrary resource.
fn same_sharepoint_tenant(host: &str, resource: &str) -> bool {
    fn tenant_label(url: &str) -> Option<String> {
        let parsed = reqwest::Url::parse(url).ok()?;
        let host = parsed.host_str()?.to_ascii_lowercase();
        let label = host.strip_suffix(".sharepoint.com")?;
        // The tenant is the leftmost label; the personal-site host adds a `-my`
        // suffix to it (`contoso-my`), which is the same tenant as `contoso`.
        let first = label.split('.').next().unwrap_or(label);
        Some(first.strip_suffix("-my").unwrap_or(first).to_owned())
    }

    match (tenant_label(host), tenant_label(resource)) {
        (Some(a), Some(b)) => !a.is_empty() && a == b,
        _ => false,
    }
}

/// A connected OneDrive client holding a valid access token.
pub struct OneDriveClient {
    http: reqwest::Client,
    access_token: String,
    /// The drive item id of the folder new exports are created in; `None` is the
    /// drive root.
    root_folder_id: Option<String>,
}

impl OneDriveClient {
    /// Creates a client from an already-valid access token.
    pub fn new(
        http: reqwest::Client,
        access_token: String,
        root_folder_id: Option<String>,
    ) -> Self {
        Self {
            http,
            access_token,
            root_folder_id,
        }
    }
}

#[async_trait::async_trait]
impl FileServiceClient for OneDriveClient {
    async fn verify(&self) -> Result<()> {
        self.http
            .get(format!("{API_BASE}/me/drive"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // /content answers 302 to a pre-authenticated URL; reqwest follows it and
        // drops the bearer header on the cross-host hop (the target rejects it).
        // Encode the id into the path segment so a stored key can never alter the
        // request URL.
        let id = encode_path_segment(id);
        let response = self
            .http
            .get(format!("{API_BASE}/me/drive/items/{id}/content"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()> {
        let FileUpload {
            name,
            content_type,
            content_length,
            body,
        } = upload;
        // Simple PUT upload of a new file into the configured folder (or root),
        // addressed by parent id and file name. Both are encoded into the path so
        // a `?`, `#`, or `/` in the name cannot retarget the request URL. The body
        // is streamed with an explicit length (the whole body is the file's bytes).
        let name = encode_path_segment(name);
        let url = match &self.root_folder_id {
            Some(id) => {
                let id = encode_path_segment(id);
                format!("{API_BASE}/me/drive/items/{id}:/{name}:/content")
            }
            None => format!("{API_BASE}/me/drive/root:/{name}:/content"),
        };
        self.http
            .put(url)
            .bearer_auth(&self.access_token)
            .header("Content-Type", content_type)
            .header(CONTENT_LENGTH, content_length)
            .body(reqwest::Body::wrap_stream(body))
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::same_sharepoint_tenant;

    #[test]
    fn accepts_same_tenant_and_personal_site() {
        let host = "https://contoso-my.sharepoint.com";
        // The resolved host itself.
        assert!(same_sharepoint_tenant(
            host,
            "https://contoso-my.sharepoint.com"
        ));
        // The tenant's SharePoint root is the same tenant.
        assert!(same_sharepoint_tenant(
            host,
            "https://contoso.sharepoint.com"
        ));
        // A path on the resource does not change the tenant.
        assert!(same_sharepoint_tenant(
            host,
            "https://contoso.sharepoint.com/sites/x"
        ));
    }

    #[test]
    fn rejects_other_tenant_and_non_sharepoint() {
        let host = "https://contoso-my.sharepoint.com";
        assert!(!same_sharepoint_tenant(
            host,
            "https://evil-my.sharepoint.com"
        ));
        assert!(!same_sharepoint_tenant(host, "https://evil.sharepoint.com"));
        assert!(!same_sharepoint_tenant(host, "https://contoso.example.com"));
        assert!(!same_sharepoint_tenant(host, "https://attacker.com"));
        assert!(!same_sharepoint_tenant(host, "not a url"));
    }
}
