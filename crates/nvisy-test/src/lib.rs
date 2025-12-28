#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod mock;

pub use mock::{
    MockEmbeddingConfig, MockEmbeddingProvider, MockLanguageConfig, MockLanguageProvider,
    MockOpticalConfig, MockOpticalProvider, create_mock_embedding_service, create_mock_ocr_service,
    create_mock_services, create_mock_vlm_service,
};
