//! Virus scan status enumeration for file security scanning results.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the result of a virus scan performed on an uploaded file.
///
/// This enumeration corresponds to the `VIRUS_SCAN_STATUS` PostgreSQL enum and is used
/// to track the security status of files after they have been scanned for malware,
/// viruses, and other security threats.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::VirusScanStatus"]
pub enum VirusScanStatus {
    /// Scan pending - file has not been scanned yet
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    Pending,

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
    /// Returns whether the file is safe to use (virus scan passed).
    #[inline]
    pub fn is_safe(self) -> bool {
        matches!(self, VirusScanStatus::Clean)
    }

    /// Returns whether the file is dangerous and should be blocked.
    #[inline]
    pub fn is_unsafe(self) -> bool {
        matches!(
            self,
            VirusScanStatus::Suspicious | VirusScanStatus::Infected
        )
    }

    /// Returns whether the virus scan status is unknown or inconclusive.
    #[inline]
    pub fn is_conclusive(self) -> bool {
        !matches!(self, VirusScanStatus::Pending | VirusScanStatus::Unknown)
    }

    /// Returns whether a rescan might be beneficial.
    #[inline]
    pub fn should_rescan(self) -> bool {
        matches!(
            self,
            VirusScanStatus::Pending | VirusScanStatus::Unknown | VirusScanStatus::Suspicious
        )
    }

    #[inline]
    pub fn requires_review(self) -> bool {
        matches!(self, VirusScanStatus::Suspicious | VirusScanStatus::Unknown)
    }
}
