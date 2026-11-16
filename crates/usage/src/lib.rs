//! Usage limits read from an app's *own* signed-in session.
//!
//! Every lookup is scoped to one profile folder: the token comes from that folder's encrypted
//! cache and must belong to the account that folder says is signed in. Tokens from other
//! profiles, the CLI or the app's default folder are never used as a fallback.
//!
//! The one exception is a command-line sign-in the user links to an instance by hand (`codex`,
//! `grok_cli`, `agy`, sharing `cli`): those read the CLI's own sign-in on this computer.

pub mod agy;
pub mod claude;
pub mod cli;
pub mod codex;
mod crypto;
pub mod cursor;
pub mod grok_cli;
pub mod model;

pub use model::*;
/// Re-exported so callers build the HTTP client with the same version.
pub use reqwest;
