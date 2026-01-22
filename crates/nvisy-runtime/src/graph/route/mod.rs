//! Compiled routing types for conditional data flow.

mod file_category;
mod language;

pub use file_category::FileCategoryEvaluator;
pub use language::LanguageEvaluator;

use nvisy_dal::AnyDataValue;

use crate::definition::SwitchDef;

/// Compiled switch node - evaluates conditions and returns true/false.
#[derive(Debug, Clone)]
pub struct CompiledSwitch {
    /// The evaluator for this switch.
    evaluator: SwitchEvaluator,
}

/// Evaluator enum for switch conditions.
#[derive(Debug, Clone)]
pub enum SwitchEvaluator {
    /// Evaluate by file category (extension).
    FileCategory(FileCategoryEvaluator),
    /// Evaluate by detected language.
    Language(LanguageEvaluator),
}

impl SwitchEvaluator {
    /// Evaluates the condition against the data.
    pub fn evaluate(&self, data: &AnyDataValue) -> bool {
        match self {
            SwitchEvaluator::FileCategory(e) => e.evaluate(data),
            SwitchEvaluator::Language(e) => e.evaluate(data),
        }
    }
}

impl CompiledSwitch {
    /// Creates a new compiled switch.
    pub fn new(evaluator: SwitchEvaluator) -> Self {
        Self { evaluator }
    }

    /// Evaluates the switch condition against input data.
    ///
    /// Returns `true` if the condition matches, `false` otherwise.
    pub fn evaluate(&self, data: &AnyDataValue) -> bool {
        self.evaluator.evaluate(data)
    }
}

impl From<SwitchDef> for CompiledSwitch {
    fn from(def: SwitchDef) -> Self {
        use crate::definition::SwitchCondition;

        let evaluator = match def.condition {
            SwitchCondition::FileCategory(c) => {
                SwitchEvaluator::FileCategory(FileCategoryEvaluator::new(c.category))
            }
            SwitchCondition::Language(c) => {
                SwitchEvaluator::Language(LanguageEvaluator::new(c.codes, c.min_confidence))
            }
        };

        Self::new(evaluator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{FileCategory, FileCategoryCondition, SwitchCondition};

    #[test]
    fn test_compiled_switch_from_def() {
        let def = SwitchDef::new(SwitchCondition::FileCategory(FileCategoryCondition {
            category: FileCategory::Image,
        }));

        let switch = CompiledSwitch::from(def);

        let jpg = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("photo.jpg", vec![]));
        let pdf = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("doc.pdf", vec![]));

        assert!(switch.evaluate(&jpg));
        assert!(!switch.evaluate(&pdf));
    }
}
