#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalUsername {
    pub canonical: String,
    pub lookup_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalUsernameError {
    Empty,
}

pub fn canonicalize_username(raw: &str) -> Result<CanonicalUsername, CanonicalUsernameError> {
    let trimmed = raw.trim();
    let without_leading_at = trimmed.strip_prefix('@').unwrap_or(trimmed);
    let canonical = without_leading_at.trim();

    if canonical.is_empty() {
        return Err(CanonicalUsernameError::Empty);
    }

    Ok(CanonicalUsername {
        canonical: canonical.to_string(),
        lookup_key: canonical.to_lowercase(),
    })
}

pub fn username_lookup_key(raw: &str) -> Option<String> {
    canonicalize_username(raw)
        .ok()
        .map(|value| value.lookup_key)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn usernames_match_for_lookup(lhs: &str, rhs: &str) -> bool {
    match (username_lookup_key(lhs), username_lookup_key(rhs)) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_username, usernames_match_for_lookup, CanonicalUsernameError};

    #[test]
    fn canonicalize_username_strips_one_leading_at_and_whitespace() {
        assert_eq!(
            canonicalize_username("  @Shop_ABC  ").unwrap().canonical,
            "Shop_ABC"
        );
        assert_eq!(
            canonicalize_username("@@Shop_ABC").unwrap().canonical,
            "@Shop_ABC"
        );
        assert_eq!(
            canonicalize_username("shop_abc").unwrap().lookup_key,
            "shop_abc"
        );
    }

    #[test]
    fn canonicalize_username_rejects_empty_after_cleanup() {
        assert_eq!(
            canonicalize_username(" @ ").unwrap_err(),
            CanonicalUsernameError::Empty
        );
    }

    #[test]
    fn usernames_match_for_lookup_uses_canonical_rule() {
        assert!(usernames_match_for_lookup(" @Shop_ABC ", "shop_abc"));
        assert!(!usernames_match_for_lookup("@@Shop_ABC", "@shop_abc"));
        assert!(!usernames_match_for_lookup(" @ ", "shop_abc"));
    }
}
