use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

/// Processing stage for objects in the Nvisy system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Input documents awaiting processing.
    Input,
    /// Documents in intermediate processing stages.
    Intermediate,
    /// Final processed output documents.
    Output,
}

impl Stage {
    /// Returns whether this is an input stage.
    pub fn is_input(&self) -> bool {
        matches!(self, Stage::Input)
    }

    /// Returns whether this is an intermediate stage.
    pub fn is_intermediate(&self) -> bool {
        matches!(self, Stage::Intermediate)
    }

    /// Returns whether this is an output stage.
    pub fn is_output(&self) -> bool {
        matches!(self, Stage::Output)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Stage::Input | Stage::Output)
    }
}
