#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
pub mod handler;
pub mod service;

pub use error::{Result, WorkerError};
pub use handler::{PostprocessingWorker, PreprocessingWorker, ProcessingWorker, WorkerHandles};
pub use service::{TextSplitterConfig, TextSplitterService, WorkerConfig, WorkerState};
