//! # Locators — how a block finds its element on the page
//!
//! Purpose: one way to point at a DOM element that survives layout changes
//!          between desktop and mobile (HANDOFF §8.1 "target").
//! Solves:  pixel coordinates break on every viewport; selectors don't. A
//!          block lists *several* locators, and the first one that matches
//!          exactly one element wins. If a class name changes, the next
//!          locator still works.
//! Priority (most robust first): `sr` (`data-sr` attribute, HANDOFF D-16) →
//!          `testid` → `css` → `role`+`name` → `text` → `xpath`.

use serde::{Deserialize, Serialize};

/// One way to find an element. Exactly one strategy field must be set
/// (`name` only together with `role`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Locator {
    /// Value of a `data-sr="…"` attribute — the recommended way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<String>,
    /// Value of a `data-testid="…"` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub testid: Option<String>,
    /// CSS selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
    /// ARIA role (`button`, `link`, `heading`, `navigation`…), optionally with `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Accessible name for `role` (aria-label or visible text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Visible text of the element (whitespace-insensitive, exact match first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// XPath expression — last resort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xpath: Option<String>,
    /// Pick the n-th match (0-based) when the locator matches several elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nth: Option<u32>,
}

impl Locator {
    /// Locator by `data-sr` attribute.
    pub fn sr(value: &str) -> Self {
        Self {
            sr: Some(value.to_owned()),
            ..Self::default()
        }
    }

    /// Checks that exactly one strategy is set and values are non-empty.
    pub fn check(&self) -> Result<(), String> {
        let set: Vec<&str> = [
            ("sr", &self.sr),
            ("testid", &self.testid),
            ("css", &self.css),
            ("role", &self.role),
            ("text", &self.text),
            ("xpath", &self.xpath),
        ]
        .iter()
        .filter(|(_, v)| v.is_some())
        .map(|(k, _)| *k)
        .collect();
        match set.as_slice() {
            [] => {
                return Err(
                    "locator has no strategy (use sr, testid, css, role, text or xpath)".into(),
                );
            }
            [_] => {}
            many => {
                return Err(format!(
                    "locator mixes strategies {many:?}; use one per locator"
                ));
            }
        }
        if self.name.is_some() && self.role.is_none() {
            return Err("locator 'name' only works together with 'role'".into());
        }
        let empty = [
            &self.sr,
            &self.testid,
            &self.css,
            &self.role,
            &self.text,
            &self.xpath,
        ]
        .iter()
        .any(|v| v.as_deref().is_some_and(|s| s.trim().is_empty()));
        if empty {
            return Err("locator value is empty".into());
        }
        Ok(())
    }

    /// Short human description, e.g. `sr=hero` or `role=link[Pricing]`.
    pub fn describe(&self) -> String {
        let base = if let Some(v) = &self.sr {
            format!("sr={v}")
        } else if let Some(v) = &self.testid {
            format!("testid={v}")
        } else if let Some(v) = &self.css {
            format!("css={v}")
        } else if let Some(v) = &self.role {
            match &self.name {
                Some(n) => format!("role={v}[{n}]"),
                None => format!("role={v}"),
            }
        } else if let Some(v) = &self.text {
            format!("text=\"{v}\"")
        } else if let Some(v) = &self.xpath {
            format!("xpath={v}")
        } else {
            "<empty>".to_owned()
        };
        match self.nth {
            Some(n) => format!("{base} #{n}"),
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_strategy() {
        assert!(Locator::sr("hero").check().is_ok());
        assert!(Locator::default().check().is_err());
        let mixed = Locator {
            css: Some(".a".into()),
            text: Some("b".into()),
            ..Locator::default()
        };
        assert!(mixed.check().is_err());
        let name_only = Locator {
            name: Some("x".into()),
            ..Locator::default()
        };
        assert!(name_only.check().is_err());
        let role = Locator {
            role: Some("link".into()),
            name: Some("Pricing".into()),
            ..Locator::default()
        };
        assert!(role.check().is_ok());
        assert_eq!(role.describe(), "role=link[Pricing]");
    }
}
