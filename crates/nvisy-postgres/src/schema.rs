// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "activity_type"))]
    pub struct ActivityType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "api_token_type"))]
    pub struct ApiTokenType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "artifact_type"))]
    pub struct ArtifactType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "file_source"))]
    pub struct FileSource;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "invite_status"))]
    pub struct InviteStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "notification_event"))]
    pub struct NotificationEvent;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "pipeline_run_status"))]
    pub struct PipelineRunStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "pipeline_status"))]
    pub struct PipelineStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "pipeline_trigger_type"))]
    pub struct PipelineTriggerType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "sync_status"))]
    pub struct SyncStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "sync_trigger_type"))]
    pub struct SyncTriggerType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "webhook_event"))]
    pub struct WebhookEvent;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "webhook_status"))]
    pub struct WebhookStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "workspace_role"))]
    pub struct WorkspaceRole;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ApiTokenType;

    account_api_tokens (id) {
        id -> Uuid,
        account_id -> Uuid,
        name -> Text,
        session_type -> ApiTokenType,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        is_remembered -> Bool,
        issued_at -> Timestamptz,
        expired_at -> Nullable<Timestamptz>,
        last_used_at -> Nullable<Timestamptz>,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::NotificationEvent;

    account_notifications (id) {
        id -> Uuid,
        account_id -> Uuid,
        notify_type -> NotificationEvent,
        title -> Text,
        message -> Text,
        is_read -> Bool,
        read_at -> Nullable<Timestamptz>,
        related_id -> Nullable<Uuid>,
        related_type -> Nullable<Text>,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        expires_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    accounts (id) {
        id -> Uuid,
        is_admin -> Bool,
        is_verified -> Bool,
        is_suspended -> Bool,
        display_name -> Text,
        email_address -> Text,
        password_hash -> Text,
        company_name -> Nullable<Text>,
        avatar_url -> Nullable<Text>,
        timezone -> Text,
        locale -> Text,
        password_changed_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ActivityType;

    workspace_activities (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Nullable<Uuid>,
        activity_type -> ActivityType,
        description -> Text,
        metadata -> Jsonb,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SyncTriggerType;
    use super::sql_types::SyncStatus;

    workspace_connection_runs (id) {
        id -> Uuid,
        connection_id -> Uuid,
        account_id -> Nullable<Uuid>,
        trigger_type -> SyncTriggerType,
        status -> SyncStatus,
        run_number -> Int4,
        records_synced -> Int8,
        error_message -> Nullable<Text>,
        metadata -> Jsonb,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_connections (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        slug -> Text,
        name -> Text,
        provider -> Text,
        encrypted_data -> Bytea,
        is_active -> Bool,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_contexts (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        slug -> Text,
        name -> Text,
        description -> Nullable<Text>,
        version -> Text,
        definition -> Bytea,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::FileSource;

    workspace_files (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        parent_id -> Nullable<Uuid>,
        version_number -> Int4,
        display_name -> Text,
        original_filename -> Text,
        file_extension -> Text,
        mime_type -> Nullable<Text>,
        tags -> Array<Nullable<Text>>,
        source -> FileSource,
        file_size_bytes -> Int8,
        file_hash_sha256 -> Bytea,
        storage_path -> Text,
        storage_bucket -> Text,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::WorkspaceRole;
    use super::sql_types::InviteStatus;

    workspace_invites (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        invitee_email -> Nullable<Text>,
        invited_role -> WorkspaceRole,
        invite_token -> Text,
        invite_status -> InviteStatus,
        expires_at -> Timestamptz,
        responded_at -> Nullable<Timestamptz>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::WorkspaceRole;
    use super::sql_types::NotificationEvent;

    workspace_members (workspace_id, account_id) {
        workspace_id -> Uuid,
        account_id -> Uuid,
        member_role -> WorkspaceRole,
        notify_via_email -> Bool,
        notification_events_app -> Array<Nullable<NotificationEvent>>,
        notification_events_email -> Array<Nullable<NotificationEvent>>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ArtifactType;

    workspace_pipeline_artifacts (id) {
        id -> Uuid,
        run_id -> Uuid,
        file_id -> Uuid,
        artifact_type -> ArtifactType,
        metadata -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_pipeline_contexts (pipeline_id, context_id) {
        workspace_id -> Uuid,
        pipeline_id -> Uuid,
        context_id -> Uuid,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_pipeline_policies (pipeline_id, policy_id) {
        workspace_id -> Uuid,
        pipeline_id -> Uuid,
        policy_id -> Uuid,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PipelineTriggerType;
    use super::sql_types::PipelineRunStatus;

    workspace_pipeline_runs (id) {
        id -> Uuid,
        pipeline_id -> Uuid,
        file_id -> Uuid,
        account_id -> Nullable<Uuid>,
        trigger_type -> PipelineTriggerType,
        status -> PipelineRunStatus,
        run_number -> Int4,
        analyzed_document_key -> Nullable<Text>,
        idempotency_key -> Nullable<Text>,
        metadata -> Jsonb,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PipelineStatus;

    workspace_pipelines (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        slug -> Text,
        name -> Text,
        description -> Nullable<Text>,
        status -> PipelineStatus,
        definition -> Jsonb,
        metadata -> Jsonb,
        schedule_cron -> Nullable<Text>,
        schedule_tz -> Nullable<Text>,
        next_run_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_policies (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        slug -> Text,
        name -> Text,
        description -> Nullable<Text>,
        version -> Text,
        definition -> Bytea,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::WebhookEvent;
    use super::sql_types::WebhookStatus;

    workspace_webhooks (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        slug -> Text,
        display_name -> Text,
        description -> Text,
        url -> Text,
        events -> Array<Nullable<WebhookEvent>>,
        headers -> Jsonb,
        encrypted_secret -> Bytea,
        status -> WebhookStatus,
        last_triggered_at -> Nullable<Timestamptz>,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspaces (id) {
        id -> Uuid,
        display_name -> Text,
        slug -> Text,
        description -> Nullable<Text>,
        avatar_url -> Nullable<Text>,
        require_approval -> Bool,
        tags -> Array<Nullable<Text>>,
        metadata -> Jsonb,
        settings -> Jsonb,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::joinable!(account_api_tokens -> accounts (account_id));
diesel::joinable!(account_notifications -> accounts (account_id));
diesel::joinable!(workspace_activities -> accounts (account_id));
diesel::joinable!(workspace_activities -> workspaces (workspace_id));
diesel::joinable!(workspace_connection_runs -> accounts (account_id));
diesel::joinable!(workspace_connection_runs -> workspace_connections (connection_id));
diesel::joinable!(workspace_connections -> accounts (account_id));
diesel::joinable!(workspace_connections -> workspaces (workspace_id));
diesel::joinable!(workspace_contexts -> accounts (account_id));
diesel::joinable!(workspace_contexts -> workspaces (workspace_id));
diesel::joinable!(workspace_files -> accounts (account_id));
diesel::joinable!(workspace_files -> workspaces (workspace_id));
diesel::joinable!(workspace_invites -> workspaces (workspace_id));
diesel::joinable!(workspace_members -> workspaces (workspace_id));
diesel::joinable!(workspace_pipeline_artifacts -> workspace_files (file_id));
diesel::joinable!(workspace_pipeline_artifacts -> workspace_pipeline_runs (run_id));
diesel::joinable!(workspace_pipeline_contexts -> workspaces (workspace_id));
diesel::joinable!(workspace_pipeline_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_pipeline_runs -> accounts (account_id));
diesel::joinable!(workspace_pipeline_runs -> workspace_files (file_id));
diesel::joinable!(workspace_pipeline_runs -> workspace_pipelines (pipeline_id));
diesel::joinable!(workspace_pipelines -> accounts (account_id));
diesel::joinable!(workspace_pipelines -> workspaces (workspace_id));
diesel::joinable!(workspace_policies -> accounts (account_id));
diesel::joinable!(workspace_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_webhooks -> accounts (created_by));
diesel::joinable!(workspace_webhooks -> workspaces (workspace_id));
diesel::joinable!(workspaces -> accounts (created_by));

diesel::allow_tables_to_appear_in_same_query!(
    account_api_tokens,
    account_notifications,
    accounts,
    workspace_activities,
    workspace_connection_runs,
    workspace_connections,
    workspace_contexts,
    workspace_files,
    workspace_invites,
    workspace_members,
    workspace_pipeline_artifacts,
    workspace_pipeline_contexts,
    workspace_pipeline_policies,
    workspace_pipeline_runs,
    workspace_pipelines,
    workspace_policies,
    workspace_webhooks,
    workspaces,
);
