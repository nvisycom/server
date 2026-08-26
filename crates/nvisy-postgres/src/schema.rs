// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "activity_type"))]
    pub struct ActivityType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "api_token_type"))]
    pub struct ApiTokenType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "chat_role"))]
    pub struct ChatRole;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "detection_status"))]
    pub struct DetectionStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "file_kind"))]
    pub struct FileKind;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "invite_status"))]
    pub struct InviteStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "notification_event"))]
    pub struct NotificationEvent;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "outbox_status"))]
    pub struct OutboxStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "pipeline_status"))]
    pub struct PipelineStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "pipeline_trigger_type"))]
    pub struct PipelineTriggerType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "provider_type"))]
    pub struct ProviderType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "sync_deletion_policy"))]
    pub struct SyncDeletionPolicy;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "sync_mode"))]
    pub struct SyncMode;

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
        display_name -> Text,
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
        read_at -> Nullable<Timestamptz>,
        params -> Jsonb,
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
        username -> Text,
        display_name -> Nullable<Text>,
        email_address -> Text,
        password_hash -> Text,
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
    use super::sql_types::ChatRole;

    chat_messages (id) {
        id -> Uuid,
        session_id -> Uuid,
        parent_id -> Nullable<Uuid>,
        role -> ChatRole,
        content -> Bytea,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    chat_sessions (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        title -> Text,
        current_message_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::OutboxStatus;

    event_outbox (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        event -> Jsonb,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        status -> OutboxStatus,
        attempts -> Int4,
        next_attempt_at -> Timestamptz,
        created_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ActivityType;

    workspace_activities (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        activity_type -> ActivityType,
        params -> Jsonb,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SyncMode;
    use super::sql_types::SyncDeletionPolicy;

    workspace_connection_schedule (connection_id) {
        connection_id -> Uuid,
        sync_mode -> SyncMode,
        schedule_cron -> Nullable<Text>,
        deletion_policy -> SyncDeletionPolicy,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SyncTriggerType;
    use super::sql_types::SyncStatus;

    workspace_connection_syncs (id) {
        id -> Uuid,
        connection_id -> Uuid,
        account_id -> Uuid,
        trigger_type -> SyncTriggerType,
        status -> SyncStatus,
        records_synced -> Int8,
        attempt -> Int4,
        error_message -> Nullable<Text>,
        metadata -> Jsonb,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ProviderType;

    workspace_connections (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        display_name -> Text,
        provider -> Text,
        provider_type -> ProviderType,
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

    workspace_detection_usage (id) {
        id -> Uuid,
        detection_id -> Uuid,
        model -> Text,
        version -> Nullable<Text>,
        input_tokens -> Nullable<Int8>,
        output_tokens -> Nullable<Int8>,
        total_tokens -> Nullable<Int8>,
        duration_ms -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PipelineTriggerType;
    use super::sql_types::DetectionStatus;

    workspace_detections (id) {
        id -> Uuid,
        pipeline_id -> Uuid,
        account_id -> Uuid,
        input_file_id -> Uuid,
        audit_file_id -> Nullable<Uuid>,
        trigger_type -> PipelineTriggerType,
        status -> DetectionStatus,
        idempotency_key -> Nullable<Text>,
        metadata -> Jsonb,
        claimed_at -> Nullable<Timestamptz>,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_file_imports (file_id) {
        file_id -> Uuid,
        connection_id -> Uuid,
        source_key -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::FileKind;

    workspace_files (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        parent_id -> Nullable<Uuid>,
        version_number -> Int4,
        display_name -> Text,
        original_filename -> Text,
        file_extension -> Text,
        file_kind -> FileKind,
        file_size_bytes -> Int8,
        file_hash_sha256 -> Bytea,
        storage_path -> Text,
        storage_bucket -> Text,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
        expires_at -> Nullable<Timestamptz>,
        purged_at -> Nullable<Timestamptz>,
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

    workspace_pipeline_policies (pipeline_id, policy_id) {
        workspace_id -> Uuid,
        pipeline_id -> Uuid,
        policy_id -> Uuid,
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
        display_name -> Text,
        description -> Nullable<Text>,
        status -> PipelineStatus,
        definition -> Jsonb,
        metadata -> Jsonb,
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
        display_name -> Text,
        description -> Nullable<Text>,
        definition -> Bytea,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_redactions (id) {
        id -> Uuid,
        detection_id -> Uuid,
        account_id -> Uuid,
        review_file_id -> Nullable<Uuid>,
        output_file_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
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
        encrypted_secret -> Bytea,
        status -> WebhookStatus,
        last_success_at -> Nullable<Timestamptz>,
        last_failure_at -> Nullable<Timestamptz>,
        consecutive_failures -> Int4,
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
diesel::joinable!(chat_sessions -> accounts (account_id));
diesel::joinable!(chat_sessions -> workspaces (workspace_id));
diesel::joinable!(event_outbox -> accounts (account_id));
diesel::joinable!(event_outbox -> workspaces (workspace_id));
diesel::joinable!(workspace_activities -> accounts (account_id));
diesel::joinable!(workspace_activities -> workspaces (workspace_id));
diesel::joinable!(workspace_connection_schedule -> workspace_connections (connection_id));
diesel::joinable!(workspace_connection_syncs -> accounts (account_id));
diesel::joinable!(workspace_connection_syncs -> workspace_connection_schedule (connection_id));
diesel::joinable!(workspace_connections -> accounts (account_id));
diesel::joinable!(workspace_connections -> workspaces (workspace_id));
diesel::joinable!(workspace_detection_usage -> workspace_detections (detection_id));
diesel::joinable!(workspace_detections -> accounts (account_id));
diesel::joinable!(workspace_detections -> workspace_pipelines (pipeline_id));
diesel::joinable!(workspace_file_imports -> workspace_connections (connection_id));
diesel::joinable!(workspace_file_imports -> workspace_files (file_id));
diesel::joinable!(workspace_files -> accounts (account_id));
diesel::joinable!(workspace_files -> workspaces (workspace_id));
diesel::joinable!(workspace_invites -> workspaces (workspace_id));
diesel::joinable!(workspace_members -> workspaces (workspace_id));
diesel::joinable!(workspace_pipeline_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_pipelines -> accounts (account_id));
diesel::joinable!(workspace_pipelines -> workspaces (workspace_id));
diesel::joinable!(workspace_policies -> accounts (account_id));
diesel::joinable!(workspace_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_redactions -> accounts (account_id));
diesel::joinable!(workspace_redactions -> workspace_detections (detection_id));
diesel::joinable!(workspace_webhooks -> accounts (created_by));
diesel::joinable!(workspace_webhooks -> workspaces (workspace_id));
diesel::joinable!(workspaces -> accounts (created_by));

diesel::allow_tables_to_appear_in_same_query!(
    account_api_tokens,
    account_notifications,
    accounts,
    chat_messages,
    chat_sessions,
    event_outbox,
    workspace_activities,
    workspace_connection_schedule,
    workspace_connection_syncs,
    workspace_connections,
    workspace_detection_usage,
    workspace_detections,
    workspace_file_imports,
    workspace_files,
    workspace_invites,
    workspace_members,
    workspace_pipeline_policies,
    workspace_pipelines,
    workspace_policies,
    workspace_redactions,
    workspace_webhooks,
    workspaces,
);
