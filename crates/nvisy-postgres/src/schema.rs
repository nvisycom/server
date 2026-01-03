// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "action_token_type"))]
    pub struct ActionTokenType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "annotation_type"))]
    pub struct AnnotationType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "activity_type"))]
    pub struct ActivityType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "api_token_type"))]
    pub struct ApiTokenType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "content_segmentation"))]
    pub struct ContentSegmentation;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "document_status"))]
    pub struct DocumentStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "integration_status"))]
    pub struct IntegrationStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "integration_type"))]
    pub struct IntegrationType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "invite_status"))]
    pub struct InviteStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "notification_event"))]
    pub struct NotificationEvent;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "processing_status"))]
    pub struct ProcessingStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "require_mode"))]
    pub struct RequireMode;

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
    use pgvector::sql_types::*;
    use super::sql_types::ActionTokenType;

    account_action_tokens (account_id, action_token) {
        action_token -> Uuid,
        account_id -> Uuid,
        action_type -> ActionTokenType,
        action_data -> Jsonb,
        ip_address -> Inet,
        user_agent -> Text,
        device_id -> Nullable<Text>,
        attempt_count -> Int4,
        max_attempts -> Int4,
        issued_at -> Timestamptz,
        expired_at -> Timestamptz,
        used_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::ApiTokenType;

    account_api_tokens (id) {
        id -> Uuid,
        account_id -> Uuid,
        name -> Text,
        session_type -> ApiTokenType,
        ip_address -> Inet,
        user_agent -> Text,
        is_remembered -> Bool,
        issued_at -> Timestamptz,
        expired_at -> Nullable<Timestamptz>,
        last_used_at -> Nullable<Timestamptz>,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
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
    use pgvector::sql_types::*;

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
    use pgvector::sql_types::*;
    use super::sql_types::AnnotationType;

    document_annotations (id) {
        id -> Uuid,
        document_file_id -> Uuid,
        account_id -> Uuid,
        content -> Text,
        annotation_type -> AnnotationType,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    document_chunks (id) {
        id -> Uuid,
        file_id -> Uuid,
        chunk_index -> Int4,
        content_sha256 -> Bytea,
        content_size -> Int4,
        token_count -> Int4,
        embedding -> Nullable<Vector>,
        embedding_model -> Nullable<Text>,
        embedded_at -> Nullable<Timestamptz>,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    document_comments (id) {
        id -> Uuid,
        file_id -> Uuid,
        account_id -> Uuid,
        parent_comment_id -> Nullable<Uuid>,
        reply_to_account_id -> Nullable<Uuid>,
        content -> Text,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::RequireMode;
    use super::sql_types::ProcessingStatus;
    use super::sql_types::ContentSegmentation;

    document_files (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        document_id -> Nullable<Uuid>,
        account_id -> Uuid,
        parent_id -> Nullable<Uuid>,
        display_name -> Text,
        original_filename -> Text,
        file_extension -> Text,
        tags -> Array<Nullable<Text>>,
        require_mode -> RequireMode,
        processing_priority -> Int4,
        processing_status -> ProcessingStatus,
        is_indexed -> Bool,
        content_segmentation -> ContentSegmentation,
        visual_support -> Bool,
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
    use pgvector::sql_types::*;
    use super::sql_types::DocumentStatus;

    documents (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        display_name -> Text,
        description -> Nullable<Text>,
        tags -> Array<Nullable<Text>>,
        status -> DocumentStatus,
        metadata -> Jsonb,
        settings -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::ActivityType;

    workspace_activities (id) {
        id -> Int8,
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
    use pgvector::sql_types::*;
    use super::sql_types::IntegrationStatus;

    workspace_integration_runs (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        integration_id -> Nullable<Uuid>,
        account_id -> Nullable<Uuid>,
        run_name -> Text,
        run_type -> Text,
        run_status -> IntegrationStatus,
        metadata -> Jsonb,
        started_at -> Nullable<Timestamptz>,
        completed_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::IntegrationType;
    use super::sql_types::IntegrationStatus;

    workspace_integrations (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        integration_name -> Text,
        description -> Text,
        integration_type -> IntegrationType,
        metadata -> Jsonb,
        credentials -> Jsonb,
        is_active -> Bool,
        last_sync_at -> Nullable<Timestamptz>,
        sync_status -> Nullable<IntegrationStatus>,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
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
    use pgvector::sql_types::*;
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
    use pgvector::sql_types::*;
    use super::sql_types::WebhookEvent;
    use super::sql_types::WebhookStatus;

    workspace_webhooks (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        display_name -> Text,
        description -> Text,
        url -> Text,
        events -> Array<Nullable<WebhookEvent>>,
        headers -> Jsonb,
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
    use pgvector::sql_types::*;

    workspaces (id) {
        id -> Uuid,
        display_name -> Text,
        description -> Nullable<Text>,
        avatar_url -> Nullable<Text>,
        require_approval -> Bool,
        enable_comments -> Bool,
        tags -> Array<Nullable<Text>>,
        metadata -> Jsonb,
        settings -> Jsonb,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::joinable!(account_action_tokens -> accounts (account_id));
diesel::joinable!(account_api_tokens -> accounts (account_id));
diesel::joinable!(account_notifications -> accounts (account_id));
diesel::joinable!(document_annotations -> accounts (account_id));
diesel::joinable!(document_annotations -> document_files (document_file_id));
diesel::joinable!(document_chunks -> document_files (file_id));
diesel::joinable!(document_comments -> document_files (file_id));
diesel::joinable!(document_files -> accounts (account_id));
diesel::joinable!(document_files -> documents (document_id));
diesel::joinable!(document_files -> workspaces (workspace_id));
diesel::joinable!(documents -> accounts (account_id));
diesel::joinable!(documents -> workspaces (workspace_id));
diesel::joinable!(workspace_activities -> accounts (account_id));
diesel::joinable!(workspace_activities -> workspaces (workspace_id));
diesel::joinable!(workspace_integration_runs -> accounts (account_id));
diesel::joinable!(workspace_integration_runs -> workspace_integrations (integration_id));
diesel::joinable!(workspace_integration_runs -> workspaces (workspace_id));
diesel::joinable!(workspace_integrations -> accounts (created_by));
diesel::joinable!(workspace_integrations -> workspaces (workspace_id));
diesel::joinable!(workspace_invites -> workspaces (workspace_id));
diesel::joinable!(workspace_members -> workspaces (workspace_id));
diesel::joinable!(workspace_webhooks -> accounts (created_by));
diesel::joinable!(workspace_webhooks -> workspaces (workspace_id));
diesel::joinable!(workspaces -> accounts (created_by));

diesel::allow_tables_to_appear_in_same_query!(
    account_action_tokens,
    account_api_tokens,
    account_notifications,
    accounts,
    document_annotations,
    document_chunks,
    document_comments,
    document_files,
    documents,
    workspace_activities,
    workspace_integration_runs,
    workspace_integrations,
    workspace_invites,
    workspace_members,
    workspace_webhooks,
    workspaces,
);
