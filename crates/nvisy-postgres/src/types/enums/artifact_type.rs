//! Artifact type enumeration indicating the classification of pipeline run artifacts.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Classification of pipeline run artifacts.
///
/// This enumeration corresponds to the `ARTIFACT_TYPE` PostgreSQL enum and is used
/// to categorize artifacts produced during pipeline runs.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ArtifactType"]
pub enum ArtifactType {
    /// Input data for the run.
    #[db_rename = "input"]
    #[serde(rename = "input")]
    #[default]
    Input,

    /// Final output data.
    #[db_rename = "output"]
    #[serde(rename = "output")]
    Output,

    /// Intermediate data between nodes.
    #[db_rename = "intermediate"]
    #[serde(rename = "intermediate")]
    Intermediate,
}

impl ArtifactType {
    /// Returns whether this is an input artifact.
    #[inline]
    pub fn is_input(self) -> bool {
        matches!(self, ArtifactType::Input)
    }

    /// Returns whether this is an output artifact.
    #[inline]
    pub fn is_output(self) -> bool {
        matches!(self, ArtifactType::Output)
    }

    /// Returns whether this is an intermediate artifact.
    #[inline]
    pub fn is_intermediate(self) -> bool {
        matches!(self, ArtifactType::Intermediate)
    }
}
