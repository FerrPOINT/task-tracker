pub mod config;
pub mod error;
pub mod id;

pub use config::*;
pub use error::*;
pub use id::*;

use chrono::{DateTime, FixedOffset, Utc};

pub type Timestamp = DateTime<FixedOffset>;

pub fn now() -> Timestamp {
    Utc::now().into()
}
