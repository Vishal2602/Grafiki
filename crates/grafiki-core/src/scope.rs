use serde::{Deserialize, Serialize};

use crate::error::{GrafikiError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeChain(Vec<String>);

impl Scope {
    pub fn new(raw: impl AsRef<str>) -> Result<Self> {
        let raw = raw.as_ref().trim();

        if raw.is_empty() {
            return Ok(Self(String::new()));
        }

        validate_scope(raw)?;
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn chain(&self) -> ScopeChain {
        if self.0.is_empty() {
            return ScopeChain(vec![String::new()]);
        }

        let mut scopes = vec![String::new()];
        let mut current = String::new();

        for segment in self.0.split('/') {
            if current.is_empty() {
                current.push_str(segment);
            } else {
                current.push('/');
                current.push_str(segment);
            }
            scopes.push(current.clone());
        }

        ScopeChain(scopes)
    }
}

impl ScopeChain {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl TryFrom<&str> for Scope {
    type Error = GrafikiError;

    fn try_from(value: &str) -> Result<Self> {
        Scope::new(value)
    }
}

fn validate_scope(raw: &str) -> Result<()> {
    if raw.starts_with('/') || raw.ends_with('/') || raw.contains("//") {
        return Err(GrafikiError::InvalidScope(raw.to_owned()));
    }

    for segment in raw.split('/') {
        if segment.is_empty() || !segment.chars().all(is_valid_scope_char) {
            return Err(GrafikiError::InvalidScope(raw.to_owned()));
        }
    }

    Ok(())
}

fn is_valid_scope_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

#[cfg(test)]
mod tests {
    use super::Scope;

    #[test]
    fn root_scope_resolves_to_global_scope_only() {
        let scope = Scope::new("").unwrap();

        assert_eq!(scope.chain().into_vec(), vec![""]);
    }

    #[test]
    fn nested_scope_resolves_to_global_and_ancestors() {
        let scope = Scope::new("open-insurance/backend/enrichment").unwrap();

        assert_eq!(
            scope.chain().into_vec(),
            vec![
                "",
                "open-insurance",
                "open-insurance/backend",
                "open-insurance/backend/enrichment"
            ]
        );
    }

    #[test]
    fn trims_outer_whitespace() {
        let scope = Scope::new("  open-insurance/backend  ").unwrap();

        assert_eq!(scope.as_str(), "open-insurance/backend");
    }

    #[test]
    fn rejects_empty_segments() {
        assert!(Scope::new("open-insurance//backend").is_err());
        assert!(Scope::new("/open-insurance").is_err());
        assert!(Scope::new("open-insurance/").is_err());
    }

    #[test]
    fn rejects_spaces_inside_segments() {
        assert!(Scope::new("open insurance/backend").is_err());
    }
}
