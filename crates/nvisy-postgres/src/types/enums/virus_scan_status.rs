//! Virus scan status enumeration for security scanning results.
//! Virus scan status enumeration for file security scanning.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the result of a virus scan performed on an uploaded file.
///
/// This enumeration corresponds to the `VIRUS_SCAN_STATUS` PostgreSQL enum and is used
/// to track the security status of files after they have been scanned for malware,
/// viruses, and other security threats.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::VirusScanStatus"]
pub enum VirusScanStatus {
    /// No virus or malware detected - file is safe
    #[db_rename = "clean"]
    #[serde(rename = "clean")]
    Clean,

    /// Virus or malware detected - file is dangerous
    #[db_rename = "infected"]
    #[serde(rename = "infected")]
    Infected,

    /// Suspicious activity detected - file may be harmful
    #[db_rename = "suspicious"]
    #[serde(rename = "suspicious")]
    Suspicious,

    /// Unable to determine virus status - scan failed or inconclusive
    #[db_rename = "unknown"]
    #[serde(rename = "unknown")]
    #[default]
    Unknown,
}

impl VirusScanStatus {
    /// Returns whether the file is safe to use.
    #[inline]
    pub fn is_safe(self) -> bool {
        matches!(self, VirusScanStatus::Clean)
    }

    /// Returns whether the file is dangerous and should be blocked.
    #[inline]
    pub fn is_dangerous(self) -> bool {
        matches!(self, VirusScanStatus::Infected)
    }

    /// Returns whether the file requires additional review or caution.
    #[inline]
    pub fn is_suspicious(self) -> bool {
        matches!(self, VirusScanStatus::Suspicious)
    }

    /// Returns whether the virus scan status is unknown or inconclusive.
    #[inline]
    pub fn is_unknown(self) -> bool {
        matches!(self, VirusScanStatus::Unknown)
    }

    /// Returns whether the file should be allowed for processing.
    #[inline]
    pub fn allows_processing(self) -> bool {
        matches!(self, VirusScanStatus::Clean)
    }

    /// Returns whether the file should be blocked from processing.
    #[inline]
    pub fn blocks_processing(self) -> bool {
        matches!(self, VirusScanStatus::Infected)
    }

    /// Returns whether the file requires manual review before processing.
    #[inline]
    pub fn requires_review(self) -> bool {
        matches!(self, VirusScanStatus::Suspicious | VirusScanStatus::Unknown)
    }

    /// Returns whether the file should be quarantined.
    #[inline]
    pub fn requires_quarantine(self) -> bool {
        matches!(
            self,
            VirusScanStatus::Infected | VirusScanStatus::Suspicious
        )
    }

    /// Returns whether the file can be downloaded by users.
    #[inline]
    pub fn allows_download(self) -> bool {
        matches!(self, VirusScanStatus::Clean)
    }

    /// Returns whether the scan result is conclusive.
    #[inline]
    pub fn is_conclusive(self) -> bool {
        !matches!(self, VirusScanStatus::Unknown)
    }

    /// Returns whether a rescan might be beneficial.
    #[inline]
    pub fn should_rescan(self) -> bool {
        matches!(self, VirusScanStatus::Unknown | VirusScanStatus::Suspicious)
    }

    /// Returns the security level (1 = safe, 4 = dangerous).
    #[inline]
    pub fn security_level(self) -> u8 {
        match self {
            VirusScanStatus::Clean => 1,
            VirusScanStatus::Unknown => 2,
            VirusScanStatus::Suspicious => 3,
            VirusScanStatus::Infected => 4,
        }
    }

    /// Returns a description of what the scan status means.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            VirusScanStatus::Clean => "No virus or malware detected: file is safe",
            VirusScanStatus::Infected => "Virus or malware detected: file is dangerous",
            VirusScanStatus::Suspicious => "Suspicious activity detected: file may be harmful",
            VirusScanStatus::Unknown => {
                "Unable to determine virus status: scan failed or inconclusive"
            }
        }
    }

    /// Returns scan statuses that allow normal file processing.
    pub fn safe_statuses() -> &'static [VirusScanStatus] {
        &[VirusScanStatus::Clean]
    }

    /// Returns scan statuses that require administrative attention.
    pub fn dangerous_statuses() -> &'static [VirusScanStatus] {
        &[VirusScanStatus::Infected, VirusScanStatus::Suspicious]
    }

    /// Returns scan statuses that may benefit from rescanning.
    pub fn rescan_candidates() -> &'static [VirusScanStatus] {
        &[VirusScanStatus::Unknown, VirusScanStatus::Suspicious]
    }
}

impl PartialOrd for VirusScanStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VirusScanStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.security_level().cmp(&other.security_level())
    }
}
