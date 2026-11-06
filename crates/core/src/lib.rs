//! Platform-neutral domain of AI Launcher: what can be launched (installations),
//! how each account copy is isolated (instances), and how the catalog is kept on disk.
//!
//! Nothing in this crate touches windows, processes or the network. Adapters only
//! *describe* how to detect and launch an app; the app crate executes those plans.

pub mod adapter;
pub mod adapters;
pub mod catalog;
pub mod env;
pub mod model;
pub mod settings;
pub mod store;
pub mod text;

pub use adapter::{AppAdapter, Brand, Isolation, LaunchPlan, ProcessMatcher, Registry, UsageKind};
pub use env::Env;
pub use model::*;
pub use settings::*;
pub use store::{Store, StoreError, StoreFile};
