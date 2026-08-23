pub mod config;
pub mod error;
pub mod events;
pub mod id;

pub use config::*;
pub use error::*;
pub use events::*;
pub use id::*;

use chrono::{DateTime, FixedOffset, Utc};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

pub type Timestamp = DateTime<FixedOffset>;

pub fn now() -> Timestamp {
    Utc::now().into()
}
