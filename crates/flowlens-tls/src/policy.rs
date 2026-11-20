//! Per-host SNI decryption policy for the MITM proxy.
//!
//! The UI lets the operator pick which hostnames are terminated (decrypted) and
//! which are passed through untouched. Rules are stored in insertion order so the
//! dropdown keeps a stable listing, and `*.example.com` style wildcards match any
//! single-label subdomain.

use serde::{Deserialize, Serialize};

use crate::TlsError;

/// A single per-host rule as exposed to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainRule {
    /// Normalized host or wildcard pattern, e.g. `api.example.com` / `*.example.com`.
    pub domain: String,
    /// Whether the MITM proxy terminates TLS and decrypts for this host.
    pub decrypt: bool,
    /// True when the rule was entered with a `*.` prefix.
    pub wildcard: bool,
}

/// Ordered, per-host decryption policy.
#[derive(Debug, Clone, Default)]
pub struct DomainPolicy {
    rules: Vec<DomainRule>,
}

impl DomainPolicy {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add (or re-enable) a domain. Idempotent: adding an existing rule only
    /// refreshes its `decrypt` flag and preserves its position.
    pub fn add(&mut self, input: &str, decrypt: bool) -> Result<DomainRule, TlsError> {
        let domain = normalize_domain(input)?;
        let wildcard = domain.starts_with("*.");
        let rule = DomainRule {
            domain,
            decrypt,
            wildcard,
        };
        match self.rules.iter_mut().find(|r| r.domain == rule.domain) {
            Some(existing) => existing.decrypt = decrypt,
            None => self.rules.push(rule.clone()),
        }
        Ok(rule)
    }

    /// Remove a rule. Returns `true` when a matching rule was present.
    pub fn remove(&mut self, input: &str) -> bool {
        let Ok(domain) = normalize_domain(input) else {
            return false;
        };
        let before = self.rules.len();
        self.rules.retain(|r| r.domain != domain);
        self.rules.len() != before
    }

    /// Flip the `decrypt` flag of an existing rule.
    ///
    /// Returns the updated rule, or `None` when the domain is unknown.
    pub fn set_decrypt(&mut self, input: &str, decrypt: bool) -> Option<DomainRule> {
        let domain = normalize_domain(input).ok()?;
        let rule = self.rules.iter_mut().find(|r| r.domain == domain)?;
        rule.decrypt = decrypt;
        Some(rule.clone())
    }

    /// Whether a concrete hostname should be decrypted.
    ///
    /// Only hosts with an explicit enabled rule (exact or matching `*.` wildcard)
    /// are terminated; everything else is passed through.
    pub fn decrypt_enabled(&self, host: &str) -> bool {
        self.rules
            .iter()
            .any(|r| r.decrypt && matches_domain(&r.domain, host))
    }

    /// Snapshot of all rules in insertion order.
    pub fn rules(&self) -> &[DomainRule] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Accessor trait for the per-host decryption policy.
///
/// The MITM session depends only on this single read method so that any
/// policy implementation (the concrete `DomainPolicy`, a test double, or a
/// shared handle behind an `Arc`) can drive the interception decision without
/// pulling in the full mutating API.
pub trait DomainPolicyAccess {
    /// Whether a concrete hostname should be decrypted (TLS terminated).
    fn decrypt_enabled(&self, host: &str) -> bool;
}

impl DomainPolicyAccess for DomainPolicy {
    fn decrypt_enabled(&self, host: &str) -> bool {
        DomainPolicy::decrypt_enabled(self, host)
    }
}

/// Normalize and validate a hostname or wildcard pattern.
///
/// Strips surrounding whitespace and a trailing dot, lowercases, and rejects
/// anything that is not a hostname: URLs, ports, paths, wildcards other than a
/// leading `*.`, bare single labels, illegal characters, and over-long labels.
pub fn normalize_domain(input: &str) -> Result<String, TlsError> {
    let trimmed = input.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return Err(TlsError::NotSupported("domain must not be empty".into()));
    }

    let (wildcard, host) = match trimmed.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('.').unwrap_or(trimmed)),
    };

    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return Err(TlsError::NotSupported("domain must not be empty".into()));
    }
    if host.len() > 253 {
        return Err(TlsError::NotSupported(
            "domain exceeds 253 characters".into(),
        ));
    }

    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return Err(TlsError::NotSupported(format!(
            "'{input}' is not a fully-qualified domain"
        )));
    }
    for label in labels {
        let valid_len = (1..=63).contains(&label.len());
        let valid_chars = label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let no_edge_dash = !label.starts_with('-') && !label.ends_with('-');
        if !valid_len || !valid_chars || !no_edge_dash {
            return Err(TlsError::NotSupported(format!(
                "invalid label '{label}' in domain '{input}'"
            )));
        }
    }

    Ok(if wildcard { format!("*.{host}") } else { host })
}

/// Match a concrete hostname against a normalized domain or `*.` wildcard rule.
pub fn matches_domain(pattern: &str, host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
        let suffix = suffix.to_ascii_lowercase();
        // `*.example.com` matches `a.example.com` (single label) but not the apex.
        match host.strip_suffix(&format!(".{suffix}")) {
            Some(prefix) => !prefix.is_empty() && !prefix.contains('.'),
            None => false,
        }
    } else {
        host == pattern.to_ascii_lowercase()
    }
}

impl crate::DecryptPolicy for DomainPolicy {
    fn set_domain(&mut self, domain: &str, decrypt: bool) {
        let _ = self.add(domain, decrypt);
    }

    fn remove_domain(&mut self, domain: &str) -> bool {
        self.remove(domain)
    }

    fn domain_decrypt(&self, domain: &str) -> Option<bool> {
        self.rules
            .iter()
            .find(|r| r.domain == domain)
            .map(|r| r.decrypt)
    }

    fn list_domains(&self) -> Vec<(String, bool)> {
        self.rules
            .iter()
            .map(|r| (r.domain.clone(), r.decrypt))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_whitespace_and_trailing_dot() {
        assert_eq!(
            normalize_domain("  API.Example.COM. ").unwrap(),
            "api.example.com"
        );
        assert_eq!(normalize_domain("*.Example.com").unwrap(), "*.example.com");
    }

    #[test]
    fn rejects_empty_and_non_hostnames() {
        assert!(normalize_domain("   ").is_err());
        assert!(normalize_domain("http://example.com").is_err());
        assert!(normalize_domain("example.com:443").is_err());
        assert!(normalize_domain("/etc/passwd").is_err());
        assert!(normalize_domain("localhost").is_err());
        assert!(normalize_domain("exa mple.com").is_err());
    }

    #[test]
    fn rejects_bad_labels() {
        assert!(normalize_domain("-example.com").is_err());
        assert!(normalize_domain("example-.com").is_err());
        assert!(normalize_domain("exa_mple.com").is_err());
        assert!(normalize_domain(&format!("{}.com", "a".repeat(64))).is_err());
    }

    #[test]
    fn add_is_idempotent_and_keeps_position() {
        let mut policy = DomainPolicy::new();
        policy.add("b.example.com", false).unwrap();
        policy.add("a.example.com", true).unwrap();
        policy.add("A.EXAMPLE.COM", true).unwrap();

        assert_eq!(policy.len(), 2);
        assert_eq!(policy.rules()[0].domain, "b.example.com");
        assert_eq!(policy.rules()[1].domain, "a.example.com");
    }

    #[test]
    fn remove_reports_presence() {
        let mut policy = DomainPolicy::new();
        policy.add("a.example.com", true).unwrap();
        assert!(policy.remove("A.Example.com"));
        assert!(!policy.remove("a.example.com"));
        assert!(policy.is_empty());
    }

    #[test]
    fn decrypt_lookup_honours_exact_and_wildcard_rules() {
        let mut policy = DomainPolicy::new();
        policy.add("api.example.com", true).unwrap();
        policy.add("*.cdn.example.net", true).unwrap();
        policy.add("blocked.example.org", false).unwrap();

        assert!(policy.decrypt_enabled("api.example.com"));
        assert!(policy.decrypt_enabled("static.cdn.example.net"));
        assert!(!policy.decrypt_enabled("cdn.example.net"));
        assert!(!policy.decrypt_enabled("blocked.example.org"));
        assert!(!policy.decrypt_enabled("unknown.example.io"));
    }

    #[test]
    fn set_decrypt_only_updates_known_domains() {
        let mut policy = DomainPolicy::new();
        policy.add("a.example.com", true).unwrap();
        assert!(!policy.set_decrypt("a.example.com", false).unwrap().decrypt);
        assert!(policy.set_decrypt("missing.example.com", true).is_none());
        assert!(!policy.decrypt_enabled("a.example.com"));
    }

    #[test]
    fn wildcard_matches_only_single_label_subdomain() {
        let mut policy = DomainPolicy::new();
        policy.add("*.example.com", true).unwrap();

        // Single-label subdomains match.
        assert!(policy.decrypt_enabled("api.example.com"));
        assert!(policy.decrypt_enabled("WWW.Example.COM"));
        // The apex does not match a `*.` rule.
        assert!(!policy.decrypt_enabled("example.com"));
        // Nested labels do not match a single-label wildcard.
        assert!(!policy.decrypt_enabled("a.b.example.com"));
        // Unrelated host does not match.
        assert!(!policy.decrypt_enabled("example.com.evil.test"));
    }

    #[test]
    fn wildcard_matches_domain_helper_directly() {
        assert!(matches_domain("*.example.com", "api.example.com"));
        assert!(matches_domain("*.example.com", "API.EXAMPLE.COM"));
        assert!(!matches_domain("*.example.com", "example.com"));
        assert!(!matches_domain("*.example.com", "a.b.example.com"));
        assert!(matches_domain("api.example.com", "api.example.com"));
        assert!(!matches_domain("api.example.com", "www.example.com"));
    }

    #[test]
    fn decrypt_decision_respects_toggle_state() {
        // A disabled rule must never intercept, even when the domain matches.
        let mut policy = DomainPolicy::new();
        policy.add("api.example.com", true).unwrap();
        assert!(policy.decrypt_enabled("api.example.com"));
        policy.set_decrypt("api.example.com", false).unwrap();
        assert!(!policy.decrypt_enabled("api.example.com"));
    }
}
