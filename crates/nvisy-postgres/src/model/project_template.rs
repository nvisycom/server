//! Project template model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::project_templates;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Project template model representing a reusable project template.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectTemplate {
    /// Unique template identifier.
    pub id: Uuid,
    /// Human-readable template name for display.
    pub display_name: String,
    /// Optional template description.
    pub description: Option<String>,
    /// Template category/type.
    pub category: String,
    /// Whether the template is publicly available.
    pub is_public: bool,
    /// Whether the template is featured/recommended.
    pub is_featured: bool,
    /// Template configuration data (JSON).
    pub template_data: serde_json::Value,
    /// Default project settings (JSON).
    pub default_settings: serde_json::Value,
    /// Number of times template has been used.
    pub usage_count: i32,
    /// Reference to the account that created this template.
    pub created_by: Uuid,
    /// Timestamp when the template was created.
    pub created_at: Timestamp,
    /// Timestamp when the template was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the template was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new project template.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectTemplate {
    /// Display name.
    pub display_name: String,
    /// Description.
    pub description: Option<String>,
    /// Category.
    pub category: Option<String>,
    /// Is public flag.
    pub is_public: Option<bool>,
    /// Is featured flag.
    pub is_featured: Option<bool>,
    /// Template data.
    pub template_data: Option<serde_json::Value>,
    /// Default settings.
    pub default_settings: Option<serde_json::Value>,
    /// Usage count.
    pub usage_count: Option<i32>,
    /// Created by.
    pub created_by: Uuid,
}

/// Data for updating a project template.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectTemplate {
    /// Display name.
    pub display_name: Option<String>,
    /// Description.
    pub description: Option<Option<String>>,
    /// Category.
    pub category: Option<String>,
    /// Is public flag.
    pub is_public: Option<bool>,
    /// Is featured flag.
    pub is_featured: Option<bool>,
    /// Template data.
    pub template_data: Option<serde_json::Value>,
    /// Default settings.
    pub default_settings: Option<serde_json::Value>,
    /// Usage count.
    pub usage_count: Option<i32>,
}

impl ProjectTemplate {
    /// Returns whether the template was created recently.
    pub fn is_recently_created(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(24))
    }

    /// Returns whether the template is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the template is available for use.
    pub fn is_available(&self) -> bool {
        !self.is_deleted()
    }

    /// Returns whether the template is publicly accessible.
    pub fn is_publicly_available(&self) -> bool {
        self.is_public && self.is_available()
    }

    /// Returns whether the template has template data.
    pub fn has_template_data(&self) -> bool {
        !self
            .template_data
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the template has default settings.
    pub fn has_default_settings(&self) -> bool {
        !self
            .default_settings
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the template has a description.
    pub fn has_description(&self) -> bool {
        self.description
            .as_ref()
            .is_some_and(|desc| !desc.trim().is_empty())
    }

    /// Returns whether the template has been used.
    pub fn is_used(&self) -> bool {
        self.usage_count > 0
    }

    /// Returns whether the template is popular (used more than threshold).
    pub fn is_popular(&self, threshold: i32) -> bool {
        self.usage_count >= threshold
    }

    /// Returns the category in title case.
    pub fn category_title(&self) -> String {
        self.category
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns whether the template is of a specific category.
    pub fn is_category(&self, category: &str) -> bool {
        self.category.eq_ignore_ascii_case(category)
    }

    /// Returns whether the template is a general template.
    pub fn is_general(&self) -> bool {
        self.is_category("general")
    }

    /// Returns whether the template is a business template.
    pub fn is_business(&self) -> bool {
        self.is_category("business")
    }

    /// Returns whether the template is an educational template.
    pub fn is_educational(&self) -> bool {
        self.is_category("educational")
    }

    /// Returns whether the template is a personal template.
    pub fn is_personal(&self) -> bool {
        self.is_category("personal")
    }

    /// Returns the template visibility description.
    pub fn visibility_description(&self) -> &'static str {
        match (self.is_public, self.is_deleted()) {
            (true, false) => "Public",
            (false, false) => "Private",
            (_, true) => "Deleted",
        }
    }

    /// Returns the template status description.
    pub fn status_description(&self) -> &'static str {
        match (self.is_featured, self.is_public, self.is_deleted()) {
            (true, true, false) => "Featured",
            (false, true, false) => "Public",
            (_, false, false) => "Private",
            (_, _, true) => "Deleted",
        }
    }

    /// Returns a template data value by key.
    pub fn get_template_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.template_data.get(key)
    }

    /// Returns a default settings value by key.
    pub fn get_default_setting(&self, key: &str) -> Option<&serde_json::Value> {
        self.default_settings.get(key)
    }

    /// Returns whether this template can be used to create projects.
    pub fn can_create_project(&self) -> bool {
        self.is_available() && self.has_template_data()
    }

    /// Returns the usage popularity tier.
    pub fn popularity_tier(&self) -> &'static str {
        match self.usage_count {
            0 => "Unused",
            1..=10 => "Low",
            11..=50 => "Medium",
            51..=200 => "High",
            _ => "Very High",
        }
    }

    /// Increments the usage count (for use in update operations).
    pub fn increment_usage(&self) -> UpdateProjectTemplate {
        UpdateProjectTemplate {
            usage_count: Some(self.usage_count + 1),
            ..Default::default()
        }
    }
}

impl HasCreatedAt for ProjectTemplate {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for ProjectTemplate {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for ProjectTemplate {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
