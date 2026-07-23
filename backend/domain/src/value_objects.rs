use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
#[path = "value_objects/tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ArcStr(Arc<str>);

impl ArcStr {
    pub fn new(s: impl Into<Arc<str>>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ArcStr {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl From<String> for ArcStr {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl std::ops::Deref for ArcStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ArcStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ArcStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ArcStr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcStr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from(s))
    }
}

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
        self.0.as_str()
    }
}
