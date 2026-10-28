//! URL parsing utilities for FlowLens

/// Parse a URL into its components
/// Returns (host, path, optional_port)
pub fn parse_url(url: &str) -> Option<(String, String, Option<u16>)> {
    let url = url.trim();
    if url.is_empty() { return None; }

    // Only try url::Url if there's a scheme
    if url.contains("://") {
        if let Ok(parsed) = url::Url::parse(url) {
            let mut host = parsed.host()?.to_string();
            // Strip brackets from IPv6 addresses
            if host.starts_with('[') && host.ends_with(']') {
                host = host[1..host.len()-1].to_string();
            }
            let path = parsed.path().to_string();
            let port = parsed.port();
            // For root path "/", use empty string only if the original had no explicit slash
            let after_scheme = &url[url.find("://").map(|i| i + 3).unwrap_or(0)..];
            let has_explicit_path = after_scheme.contains('/');
            let path = if path == "/" && !has_explicit_path
                && parsed.query().is_none() && parsed.fragment().is_none() {
                String::new()
            } else {
                path
            };
            // Preserve query string in path for display
            let full_path = match (parsed.query(), parsed.fragment()) {
                (Some(q), Some(f)) => format!("{path}?{q}#{f}"),
                (Some(q), None) => format!("{path}?{q}"),
                (None, Some(f)) => format!("{path}#{f}"),
                _ => path,
            };
            return Some((host, full_path, port));
        }
    }

    // Try to parse as host:port/path format
    if let Some((host_part, rest)) = url.split_once('/') {
        let (host, port) = parse_host_port(host_part)?;
        return Some((host, format!("/{}", rest), port));
    }

    // Try to parse as just host or host:port
    let (host, port) = parse_host_port(url)?;
    Some((host, String::new(), port))
}

/// Parse host:port string into (host, optional_port)
fn parse_host_port(s: &str) -> Option<(String, Option<u16>)> {
    let s = s.trim();

    // Handle IPv6 addresses [::1]:8080
    if s.starts_with('[') {
        if let Some(end_bracket) = s.find(']') {
            let host = &s[1..end_bracket];
            let remaining = &s[end_bracket + 1..];
            if remaining.starts_with(':') {
                let port: u16 = remaining[1..].parse().ok()?;
                Some((host.to_string(), Some(port)))
            } else {
                Some((host.to_string(), None))
            }
        } else {
            None
        }
    } else {
        // Handle regular host:port or IPv4:port
        if let Some(colon_pos) = s.rfind(':') {
            let potential_port = &s[colon_pos + 1..];
            if potential_port.parse::<u16>().is_ok() {
                let host = &s[..colon_pos];
                Some((host.to_string(), Some(potential_port.parse().ok()?)))
            } else {
                Some((s.to_string(), None))
            }
        } else {
            Some((s.to_string(), None))
        }
    }
}

/// Truncate a URL for display, removing the scheme and showing max_len characters
pub fn truncate_url(url: &str, max_len: usize) -> String {
    let url = url.trim();

    let without_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };

    let trimmed = without_scheme.trim_start_matches('/');

    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else if max_len >= 3 {
        format!("{}...", &trimmed[..max_len - 3])
    } else if max_len > 0 {
        ".".repeat(max_len)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_https_url() {
        let result = parse_url("https://api.github.com/v4/users");
        assert_eq!(result, Some(("api.github.com".to_string(), "/v4/users".to_string(), None)));
    }

    #[test]
    fn test_parse_url_with_port() {
        let result = parse_url("http://localhost:8080/api/test");
        assert_eq!(result, Some(("localhost".to_string(), "/api/test".to_string(), Some(8080))));
    }

    #[test]
    fn test_parse_url_ipv4_with_port() {
        let result = parse_url("http://192.168.1.1:3000/health");
        assert_eq!(result, Some(("192.168.1.1".to_string(), "/health".to_string(), Some(3000))));
    }

    #[test]
    fn test_parse_url_ipv6() {
        let result = parse_url("http://[::1]:8080/test");
        assert_eq!(result, Some(("::1".to_string(), "/test".to_string(), Some(8080))));
    }

    #[test]
    fn test_parse_url_no_path() {
        let result = parse_url("https://example.com");
        assert_eq!(result, Some(("example.com".to_string(), "".to_string(), None)));
    }

    #[test]
    fn test_parse_url_root_path() {
        let result = parse_url("https://example.com/");
        assert_eq!(result, Some(("example.com".to_string(), "/".to_string(), None)));
    }

    #[test]
    fn test_parse_malformed_url_empty() {
        let result = parse_url("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_url_only_host_port() {
        let result = parse_url("example.com:9000");
        assert_eq!(result, Some(("example.com".to_string(), "".to_string(), Some(9000))));
    }

    #[test]
    fn test_parse_url_without_scheme_and_path() {
        let result = parse_url("api.example.com/v1/data");
        assert_eq!(result, Some(("api.example.com".to_string(), "/v1/data".to_string(), None)));
    }

    #[test]
    fn test_truncate_url_remove_scheme() {
        let result = truncate_url("https://api.github.com/v4", 20);
        assert_eq!(result, "api.github.com/v4");
    }

    #[test]
    fn test_truncate_url_short_enough() {
        let result = truncate_url("example.com/test", 20);
        assert_eq!(result, "example.com/test");
    }

    #[test]
    fn test_truncate_url_needs_truncation() {
        let result = truncate_url("https://very-long-domain-name-with-lots-of-characters.com/path", 20);
        assert_eq!(result.len(), 20);
        assert!(result.ends_with("..."));
        assert_eq!(result, "very-long-domain-...");
    }

    #[test]
    fn test_truncate_url_exact_length() {
        let result = truncate_url("short.com/a", 11);
        assert_eq!(result, "short.com/a");
    }

    #[test]
    fn test_truncate_url_with_port() {
        let result = truncate_url("http://localhost:8080/api/v1/users", 15);
        assert_eq!(result.len(), 15);
        assert_eq!(result, "localhost:80...");
    }

    #[test]
    fn test_truncate_url_very_short_max() {
        let result = truncate_url("https://example.com/path", 2);
        assert_eq!(result, "..");
    }

    #[test]
    fn test_parse_url_special_characters_in_path() {
        let result = parse_url("https://api.example.com/v1/search?q=test&limit=10");
        assert_eq!(result, Some(("api.example.com".to_string(), "/v1/search?q=test&limit=10".to_string(), None)));
    }
}
