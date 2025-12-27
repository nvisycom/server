#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod mock;

pub use mock::{
    MockEmbeddingConfig, MockEmbeddingProvider, MockLanguageConfig, MockLanguageProvider,
    MockOpticalConfig, MockOpticalProvider, create_mock_services,
};
