#[cfg(feature = "runtime")]
pub mod config;
pub mod events;
pub mod id;

#[cfg(feature = "runtime")]
pub use config::*;
pub use events::*;
pub use id::*;

// Fleet-shared error/envelope live in sdlc-shared (services-base); re-export
// keeps every `shared::AppError` / `shared::EventEnvelope` call site intact.
pub use sdlc_shared::resource_context;
pub use sdlc_shared::{AppError, AppResult, ErrorBody, ErrorEnvelope, EventEnvelope};

use chrono::{DateTime, FixedOffset, Utc};

#[cfg(all(test, feature = "runtime"))]
#[path = "lib_tests.rs"]
mod tests;

pub type Timestamp = DateTime<FixedOffset>;

pub fn now() -> Timestamp {
    Utc::now().into()
}
