//! Usage limits read from an app's *own* signed-in session.
//!
//! Every lookup is scoped to one profile folder: the token comes from that folder's encrypted
//! cache and must belong to the account that folder says is signed in. Tokens from other
//! profiles, the CLI or the app's default folder are never used as a fallback.

pub mod claude;
mod crypto;
pub mod model;

pub use model::*;
/// Re-exported so callers build the HTTP client with the same version.
pub use reqwest;
