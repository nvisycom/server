//! Data-retention enforcement: the worker that expires stored data whose
//! retention window has elapsed.
//!
//! The retention *rules* are value types in
//! [`nvisy_postgres::types`](nvisy_postgres::types) (`Retention`,
//! `RetentionSettings`, `RetentionOverride`, `RetentionScope`); this module holds
//! only the [`FileRetentionWorker`] that acts on them.

mod worker;

pub use worker::FileRetentionWorker;
