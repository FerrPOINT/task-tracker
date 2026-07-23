use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type ArcStr = Arc<str>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichText(pub String);

impl RichText {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RichText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RichText {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RichText {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email(ArcStr);

impl Email {
    pub fn new(email: impl Into<ArcStr>) -> Self {
        Self(email.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
