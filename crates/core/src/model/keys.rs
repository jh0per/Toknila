//! Human-facing keys: a project key like `DEMO` and ticket keys like `DEMO-3`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Why a project or ticket key was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// Project keys are 2 to 10 ASCII uppercase letters or digits, starting with a letter.
    #[error("project key {0:?} must be 2-10 characters A-Z or 0-9, starting with a letter")]
    BadProjectKey(String),
    /// Ticket keys look like `DEMO-3` with a number of at least 1.
    #[error("ticket key {0:?} must look like DEMO-3")]
    BadTicketKey(String),
}

/// Short uppercase project key, the prefix of every ticket key (`DEMO`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectKey(String);

impl ProjectKey {
    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ProjectKey {
    type Err = KeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = (2..=10).contains(&s.len())
            && s.starts_with(|c: char| c.is_ascii_uppercase())
            && s.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(KeyError::BadProjectKey(s.to_owned()))
        }
    }
}

impl TryFrom<String> for ProjectKey {
    type Error = KeyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<ProjectKey> for String {
    fn from(k: ProjectKey) -> Self {
        k.0
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Ticket key `<PROJECT>-<n>`, how people and the CLI address a ticket.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TicketKey {
    project: ProjectKey,
    number: u32,
}

impl TicketKey {
    /// Builds a key; `number` starts at 1.
    pub fn new(project: ProjectKey, number: u32) -> Result<Self, KeyError> {
        if number == 0 {
            return Err(KeyError::BadTicketKey(format!("{project}-0")));
        }
        Ok(Self { project, number })
    }

    /// The project part (`DEMO`).
    pub fn project(&self) -> &ProjectKey {
        &self.project
    }

    /// The sequence number within the project (`3`).
    pub fn number(&self) -> u32 {
        self.number
    }
}

impl FromStr for TicketKey {
    type Err = KeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || KeyError::BadTicketKey(s.to_owned());
        let (project, number) = s.rsplit_once('-').ok_or_else(bad)?;
        let project: ProjectKey = project.parse().map_err(|_| bad())?;
        if number.starts_with('0') || !number.chars().all(|c| c.is_ascii_digit()) {
            return Err(bad());
        }
        let number: u32 = number.parse().map_err(|_| bad())?;
        Self::new(project, number).map_err(|_| bad())
    }
}

impl TryFrom<String> for TicketKey {
    type Error = KeyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<TicketKey> for String {
    fn from(k: TicketKey) -> Self {
        k.to_string()
    }
}

impl fmt::Display for TicketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.project, self.number)
    }
}

/// Both keys are plain strings on the wire.
#[cfg(feature = "openapi")]
mod openapi {
    use utoipa::openapi::RefOr;
    use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};

    fn string_schema(pattern: &str, example: &str) -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .pattern(Some(pattern))
            .examples([example])
            .into()
    }

    impl utoipa::PartialSchema for super::ProjectKey {
        fn schema() -> RefOr<Schema> {
            string_schema("^[A-Z][A-Z0-9]{1,9}$", "DEMO")
        }
    }
    impl utoipa::ToSchema for super::ProjectKey {}

    impl utoipa::PartialSchema for super::TicketKey {
        fn schema() -> RefOr<Schema> {
            string_schema("^[A-Z][A-Z0-9]{1,9}-[1-9][0-9]*$", "DEMO-3")
        }
    }
    impl utoipa::ToSchema for super::TicketKey {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_key_parses_and_prints() {
        let key: TicketKey = "DEMO-3".parse().expect("valid");
        assert_eq!(key.project().as_str(), "DEMO");
        assert_eq!(key.number(), 3);
        assert_eq!(key.to_string(), "DEMO-3");
    }

    #[test]
    fn bad_ticket_keys_are_rejected() {
        for bad in [
            "demo-3", "DEMO", "DEMO-", "DEMO-0", "DEMO-03", "DEMO-x", "-3", "D-1", "DEMO-+3",
        ] {
            assert!(
                bad.parse::<TicketKey>().is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn project_key_rules() {
        assert!("P0".parse::<ProjectKey>().is_ok());
        assert!("TOKNILA".parse::<ProjectKey>().is_ok());
        for bad in ["", "D", "0DEMO", "DEMO_X", "ABCDEFGHIJK", "Demo"] {
            assert!(
                bad.parse::<ProjectKey>().is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn bad_key_in_json_is_a_deserialize_error() {
        assert!(serde_json::from_str::<TicketKey>("\"demo-1\"").is_err());
        let key: TicketKey = serde_json::from_str("\"DEMO-12\"").expect("valid");
        assert_eq!(key.number(), 12);
    }
}
