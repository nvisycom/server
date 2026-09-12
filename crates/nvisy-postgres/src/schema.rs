// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "activity_type"))]
    pub struct ActivityType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "api_token_type"))]
    pub struct ApiTokenType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "connection_type"))]
    pub struct ConnectionType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "detection_status"))]
    pub struct DetectionStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "document_kind"))]
    pub struct DocumentKind;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "identity_provider"))]
    pub struct IdentityProvider;

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
    #[diesel(postgres_type(name = "policy_kind"))]
    pub struct PolicyKind;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "provider_type"))]
    pub struct ProviderType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "review_status"))]
    pub struct ReviewStatus;

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
    #[diesel(postgres_type(name = "thread_event_kind"))]
    pub struct ThreadEventKind;

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
    use super::sql_types::IdentityProvider;

    account_identities (id) {
        id -> Uuid,
        account_id -> Uuid,
        provider -> IdentityProvider,
        secret -> Nullable<Text>,
        provider_subject -> Nullable<Text>,
        provider_email -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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
        is_verified -> Bool,
        is_suspended -> Bool,
        username -> Text,
        display_name -> Nullable<Text>,
        email_address -> Text,
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
    use super::sql_types::OutboxStatus;

    workspace_assistant_jobs (id) {
        id -> Uuid,
        comment_id -> Uuid,
        job -> Jsonb,
        status -> OutboxStatus,
        attempts -> Int4,
        next_attempt_at -> Timestamptz,
        created_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_audits (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        blob_id -> Uuid,
        detection_id -> Uuid,
        redaction_id -> Nullable<Uuid>,
        derived_from -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_blobs (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        content_hash -> Bytea,
        file_size_bytes -> Int8,
        storage_path -> Text,
        storage_bucket -> Text,
        ref_count -> Int4,
        created_at -> Timestamptz,
        expires_at -> Nullable<Timestamptz>,
        purged_at -> Nullable<Timestamptz>,
        reclaimed_at -> Nullable<Timestamptz>,
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
    use super::sql_types::ConnectionType;

    workspace_connections (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        display_name -> Text,
        provider -> Text,
        connection_type -> ConnectionType,
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
    use super::sql_types::OutboxStatus;

    workspace_detection_jobs (id) {
        id -> Uuid,
        detection_id -> Uuid,
        job -> Jsonb,
        status -> OutboxStatus,
        attempts -> Int4,
        next_attempt_at -> Timestamptz,
        created_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_detection_policy_versions (detection_id, policy_version_id) {
        detection_id -> Uuid,
        policy_version_id -> Uuid,
        workspace_id -> Uuid,
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
        input_document_id -> Uuid,
        intermediate_blob_id -> Nullable<Uuid>,
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

    workspace_document_exports (document_id, connection_id) {
        document_id -> Uuid,
        connection_id -> Uuid,
        remote_key -> Text,
        exported_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_document_imports (document_id) {
        document_id -> Uuid,
        connection_id -> Uuid,
        source_key -> Text,
        imported_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::DocumentKind;

    workspace_documents (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        blob_id -> Uuid,
        kind -> DocumentKind,
        display_name -> Text,
        original_filename -> Text,
        file_extension -> Text,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::OutboxStatus;

    workspace_event_outbox (id) {
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
    use super::sql_types::PolicyKind;

    workspace_policies (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        slug -> Text,
        display_name -> Text,
        description -> Nullable<Text>,
        current_version_id -> Nullable<Uuid>,
        kind -> PolicyKind,
        content_hash -> Nullable<Bytea>,
        metadata -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_policy_versions (id) {
        id -> Uuid,
        policy_id -> Uuid,
        workspace_id -> Uuid,
        account_id -> Uuid,
        version_number -> Int4,
        definition -> Bytea,
        metadata -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ProviderType;

    workspace_providers (id) {
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

    workspace_redactions (id) {
        id -> Uuid,
        detection_id -> Uuid,
        account_id -> Uuid,
        output_document_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    workspace_thread_comments (id) {
        id -> Uuid,
        parent_id -> Nullable<Uuid>,
        workspace_id -> Uuid,
        thread_id -> Uuid,
        author_account_id -> Uuid,
        body -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ThreadEventKind;

    workspace_thread_events (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        thread_id -> Uuid,
        kind -> ThreadEventKind,
        actor_account_id -> Nullable<Uuid>,
        target -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ReviewStatus;

    workspace_threads (id) {
        id -> Uuid,
        workspace_id -> Uuid,
        document_id -> Nullable<Uuid>,
        author_account_id -> Uuid,
        display_name -> Nullable<Text>,
        assignee_account_id -> Nullable<Uuid>,
        review_status -> Nullable<ReviewStatus>,
        closed_at -> Nullable<Timestamptz>,
        closed_by -> Nullable<Uuid>,
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
diesel::joinable!(account_identities -> accounts (account_id));
diesel::joinable!(account_notifications -> accounts (account_id));
diesel::joinable!(workspace_activities -> accounts (account_id));
diesel::joinable!(workspace_activities -> workspaces (workspace_id));
diesel::joinable!(workspace_assistant_jobs -> workspace_thread_comments (comment_id));
diesel::joinable!(workspace_audits -> workspace_blobs (blob_id));
diesel::joinable!(workspace_audits -> workspace_detections (detection_id));
diesel::joinable!(workspace_audits -> workspace_redactions (redaction_id));
diesel::joinable!(workspace_audits -> workspaces (workspace_id));
diesel::joinable!(workspace_blobs -> workspaces (workspace_id));
diesel::joinable!(workspace_connection_schedule -> workspace_connections (connection_id));
diesel::joinable!(workspace_connection_syncs -> accounts (account_id));
diesel::joinable!(workspace_connection_syncs -> workspace_connections (connection_id));
diesel::joinable!(workspace_connections -> accounts (account_id));
diesel::joinable!(workspace_connections -> workspaces (workspace_id));
diesel::joinable!(workspace_detection_jobs -> workspace_detections (detection_id));
diesel::joinable!(workspace_detection_policy_versions -> workspace_detections (detection_id));
diesel::joinable!(workspace_detection_policy_versions -> workspaces (workspace_id));
diesel::joinable!(workspace_detection_usage -> workspace_detections (detection_id));
diesel::joinable!(workspace_detections -> accounts (account_id));
diesel::joinable!(workspace_detections -> workspace_blobs (intermediate_blob_id));
diesel::joinable!(workspace_detections -> workspace_documents (input_document_id));
diesel::joinable!(workspace_detections -> workspace_pipelines (pipeline_id));
diesel::joinable!(workspace_document_exports -> workspace_connections (connection_id));
diesel::joinable!(workspace_document_exports -> workspace_documents (document_id));
diesel::joinable!(workspace_document_imports -> workspace_connections (connection_id));
diesel::joinable!(workspace_document_imports -> workspace_documents (document_id));
diesel::joinable!(workspace_documents -> accounts (account_id));
diesel::joinable!(workspace_documents -> workspace_blobs (blob_id));
diesel::joinable!(workspace_documents -> workspaces (workspace_id));
diesel::joinable!(workspace_event_outbox -> accounts (account_id));
diesel::joinable!(workspace_event_outbox -> workspaces (workspace_id));
diesel::joinable!(workspace_invites -> workspaces (workspace_id));
diesel::joinable!(workspace_members -> workspaces (workspace_id));
diesel::joinable!(workspace_pipeline_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_pipelines -> accounts (account_id));
diesel::joinable!(workspace_pipelines -> workspaces (workspace_id));
diesel::joinable!(workspace_policies -> accounts (account_id));
diesel::joinable!(workspace_policies -> workspaces (workspace_id));
diesel::joinable!(workspace_policy_versions -> accounts (account_id));
diesel::joinable!(workspace_policy_versions -> workspaces (workspace_id));
diesel::joinable!(workspace_providers -> accounts (account_id));
diesel::joinable!(workspace_providers -> workspaces (workspace_id));
diesel::joinable!(workspace_redactions -> accounts (account_id));
diesel::joinable!(workspace_redactions -> workspace_detections (detection_id));
diesel::joinable!(workspace_redactions -> workspace_documents (output_document_id));
diesel::joinable!(workspace_thread_comments -> accounts (author_account_id));
diesel::joinable!(workspace_thread_comments -> workspace_threads (thread_id));
diesel::joinable!(workspace_thread_comments -> workspaces (workspace_id));
diesel::joinable!(workspace_thread_events -> accounts (actor_account_id));
diesel::joinable!(workspace_thread_events -> workspace_threads (thread_id));
diesel::joinable!(workspace_thread_events -> workspaces (workspace_id));
diesel::joinable!(workspace_threads -> workspaces (workspace_id));
diesel::joinable!(workspace_webhooks -> accounts (created_by));
diesel::joinable!(workspace_webhooks -> workspaces (workspace_id));
diesel::joinable!(workspaces -> accounts (created_by));

diesel::allow_tables_to_appear_in_same_query!(
    account_api_tokens,
    account_identities,
    account_notifications,
    accounts,
    workspace_activities,
    workspace_assistant_jobs,
    workspace_audits,
    workspace_blobs,
    workspace_connection_schedule,
    workspace_connection_syncs,
    workspace_connections,
    workspace_detection_jobs,
    workspace_detection_policy_versions,
    workspace_detection_usage,
    workspace_detections,
    workspace_document_exports,
    workspace_document_imports,
    workspace_documents,
    workspace_event_outbox,
    workspace_invites,
    workspace_members,
    workspace_pipeline_policies,
    workspace_pipelines,
    workspace_policies,
    workspace_policy_versions,
    workspace_providers,
    workspace_redactions,
    workspace_thread_comments,
    workspace_thread_events,
    workspace_threads,
    workspace_webhooks,
    workspaces,
);
