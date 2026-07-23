use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn nil() -> Self {
                Self(Uuid::nil())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

uuid_id!(UserId);
uuid_id!(ProjectId);
uuid_id!(IssueId);
uuid_id!(CommentId);
uuid_id!(AttachmentId);
uuid_id!(LabelId);
uuid_id!(SprintId);
uuid_id!(BoardId);
uuid_id!(StatusId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectKey(Arc<str>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IssueKey {
    pub project_key: ProjectKey,
    pub number: u32,
}

impl ProjectKey {
    pub fn new(key: impl Into<Arc<str>>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProjectKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProjectKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ProjectKey::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ProjectKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.len() > 10 || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(format!("invalid project key: {}", s));
        }
        Ok(Self::new(s))
    }
}

impl Serialize for IssueKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for IssueKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        IssueKey::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl IssueKey {
    pub fn new(project_key: ProjectKey, number: u32) -> Self {
        Self {
            project_key,
            number,
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let dash = s
            .rfind('-')
            .ok_or_else(|| format!("invalid issue key: {}", s))?;
        let (project, num) = s.split_at(dash);
        let number: u32 = num
            .trim_start_matches('-')
            .parse()
            .map_err(|_| format!("invalid issue key number: {}", s))?;
        Ok(Self::new(ProjectKey::from_str(project)?, number))
    }

    pub fn to_string(&self) -> String {
        format!("{}-{}", self.project_key, self.number)
    }
}

impl fmt::Display for IssueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.project_key, self.number)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    #[default]
    Task,
    Bug,
    Story,
    Epic,
    SubTask,
}

impl FromStr for IssueType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "task" | "задача" => Ok(Self::Task),
            "bug" | "баг" => Ok(Self::Bug),
            "story" | "история" => Ok(Self::Story),
            "epic" | "эпик" => Ok(Self::Epic),
            "subtask" | "подзадача" => Ok(Self::SubTask),
            _ => Err(format!("unknown issue type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Lowest,
    Low,
    Medium,
    High,
    Highest,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Lowest => "Lowest",
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Highest => "Highest",
        }
    }
}

impl FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Lowest" => Ok(Self::Lowest),
            "Low" => Ok(Self::Low),
            "Medium" => Ok(Self::Medium),
            "High" => Ok(Self::High),
            "Highest" => Ok(Self::Highest),
            _ => Err(format!("unknown priority: {}", s)),
        }
    }
}
