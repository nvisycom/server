//! Project template repository for managing project template operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectTemplate, ProjectTemplate, UpdateProjectTemplate};
use crate::{PgError, PgResult, schema};
use crate::PgConnection;

/// Repository for project template database operations.
///
/// Handles template management including CRUD operations, public template
/// discovery, category filtering, and usage tracking.
pub trait ProjectTemplateRepository {
    /// Creates a new project template.
    fn create_project_template(
        &mut self,
        new_template: NewProjectTemplate,
    ) -> impl Future<Output = PgResult<ProjectTemplate>> + Send;

    /// Finds a project template by ID.
    fn find_project_template_by_id(
        &mut self,
        template_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectTemplate>>> + Send;

    /// Lists all public templates.
    fn list_public_templates(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Lists featured templates.
    fn list_featured_templates(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Finds templates by category.
    fn find_templates_by_category(
        &mut self,
        category: String,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Finds public templates by category.
    fn find_public_templates_by_category(
        &mut self,
        category: String,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Lists all templates created by a specific account.
    fn list_templates_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Finds popular templates (by usage count).
    fn find_popular_templates(
        &mut self,
        min_usage_count: i32,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Searches templates by name or description.
    fn search_templates(
        &mut self,
        search_term: &str,
        public_only: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectTemplate>>> + Send;

    /// Updates a project template.
    fn update_project_template(
        &mut self,
        template_id: Uuid,
        changes: UpdateProjectTemplate,
    ) -> impl Future<Output = PgResult<ProjectTemplate>> + Send;

    /// Increments the usage count for a template.
    fn increment_template_usage(
        &mut self,
        template_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectTemplate>> + Send;

    /// Soft deletes a project template.
    fn delete_project_template(
        &mut self,
        template_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Checks if a template exists with the given name.
    fn template_name_exists(
        &mut self,
        template_name: &str,
        exclude_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Gets the most popular categories with template counts.
    fn get_popular_categories(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<(String, i64)>>> + Send;

    /// Sets a template as featured or unfeatured.
    fn set_template_featured(
        &mut self,
        template_id: Uuid,
        featured: bool,
    ) -> impl Future<Output = PgResult<ProjectTemplate>> + Send;

    /// Sets a template as public or private.
    fn set_template_public(
        &mut self,
        template_id: Uuid,
        public: bool,
    ) -> impl Future<Output = PgResult<ProjectTemplate>> + Send;
}

/// Default implementation of ProjectTemplateRepository using AsyncPgConnection.
impl ProjectTemplateRepository for PgConnection {
    async fn create_project_template(
        &mut self,
        new_template: NewProjectTemplate,
    ) -> PgResult<ProjectTemplate> {
        use schema::project_templates;

        let template = diesel::insert_into(project_templates::table)
            .values(&new_template)
            .returning(ProjectTemplate::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(template)
    }

    async fn find_project_template_by_id(
        &mut self,
        template_id: Uuid,
    ) -> PgResult<Option<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let template = project_templates
            .filter(id.eq(template_id))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(template)
    }

    async fn list_public_templates(
        &mut self,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(is_public.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn list_featured_templates(
        &mut self,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(is_featured.eq(true))
            .filter(is_public.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn find_templates_by_category(
        &mut self,
        template_category: String,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(category.eq(template_category))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn find_public_templates_by_category(
        &mut self,
        template_category: String,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(category.eq(template_category))
            .filter(is_public.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn list_templates_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(created_by.eq(creator_id))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn find_popular_templates(
        &mut self,
        min_usage_count: i32,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let templates = project_templates
            .filter(usage_count.ge(min_usage_count))
            .filter(is_public.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn search_templates(
        &mut self,
        search_term: &str,
        public_only: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectTemplate>> {
        use schema::project_templates::dsl::*;

        let search_pattern = format!("%{}%", search_term);

        let mut query = project_templates
            .filter(diesel::BoolExpressionMethods::or(
                display_name.ilike(&search_pattern),
                description.ilike(&search_pattern),
            ))
            .filter(deleted_at.is_null())
            .into_boxed();

        if public_only {
            query = query.filter(is_public.eq(true));
        }

        let templates = query
            .select(ProjectTemplate::as_select())
            .order((usage_count.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    async fn update_project_template(
        &mut self,
        template_id: Uuid,
        changes: UpdateProjectTemplate,
    ) -> PgResult<ProjectTemplate> {
        use schema::project_templates::dsl::*;

        let template = diesel::update(project_templates)
            .filter(id.eq(template_id))
            .set(&changes)
            .returning(ProjectTemplate::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(template)
    }

    async fn increment_template_usage(&mut self, template_id: Uuid) -> PgResult<ProjectTemplate> {
        use schema::project_templates::dsl::*;

        let template = diesel::update(project_templates)
            .filter(id.eq(template_id))
            .set(usage_count.eq(usage_count + 1))
            .returning(ProjectTemplate::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(template)
    }

    async fn delete_project_template(&mut self, template_id: Uuid) -> PgResult<()> {
        use schema::project_templates::dsl::*;

        diesel::update(project_templates)
            .filter(id.eq(template_id))
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn template_name_exists(
        &mut self,
        template_name: &str,
        exclude_id: Option<Uuid>,
    ) -> PgResult<bool> {
        use schema::project_templates::dsl::*;

        let mut query = project_templates
            .filter(display_name.eq(template_name))
            .filter(deleted_at.is_null())
            .into_boxed();

        if let Some(exclude_template_id) = exclude_id {
            query = query.filter(id.ne(exclude_template_id));
        }

        let count = query
            .count()
            .get_result::<i64>(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    async fn get_popular_categories(&mut self, limit: i64) -> PgResult<Vec<(String, i64)>> {
        use schema::project_templates::dsl::*;

        let results = project_templates
            .filter(deleted_at.is_null())
            .group_by(category)
            .select((category, diesel::dsl::count_star()))
            .order(diesel::dsl::count_star().desc())
            .limit(limit)
            .load::<(String, i64)>(self)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    async fn set_template_featured(
        &mut self,
        template_id: Uuid,
        featured: bool,
    ) -> PgResult<ProjectTemplate> {
        use schema::project_templates::dsl::*;

        let template = diesel::update(project_templates)
            .filter(id.eq(template_id))
            .set(is_featured.eq(featured))
            .returning(ProjectTemplate::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(template)
    }

    async fn set_template_public(
        &mut self,
        template_id: Uuid,
        public: bool,
    ) -> PgResult<ProjectTemplate> {
        use schema::project_templates::dsl::*;

        let template = diesel::update(project_templates)
            .filter(id.eq(template_id))
            .set(is_public.eq(public))
            .returning(ProjectTemplate::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(template)
    }
}
