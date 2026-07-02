//! taodb — LLM's Hippocampus
//!
//! Store raw memories. Retrieve by time and space. No semantic understanding.

pub mod api;
pub mod crc;
pub mod mcp;
pub mod model;
pub mod recall;
pub mod store;
pub mod tenant;

pub use model::*;
pub use recall::{
    derive_narrative_anchor, recall_multidimensional, recall_window, recall_window_with_days,
    recall_window_with_options,
};
pub use store::Store;
pub use tenant::{ProjectConfig, Tenant, TenantManager, UserConfig};
