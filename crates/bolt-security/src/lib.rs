//! Security policies, egress allowlists, and Content Security Policy enforcement.

use thiserror::Error;

pub const ALLOWED_JAGEX_HOST: &str = "account.jagex.com";
pub const ALLOWED_AUTH_HOST: &str = "auth.jagex.com";
pub const ALLOWED_REDIRECT_HOST: &str = "secure.runescape.com";
pub const CSP_VALUE: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src http://127.0.0.1:* ws://127.0.0.1:*; frame-ancestors 'none'; base-uri 'none'; form-action 'none';";
pub const CSP_META_TAG: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src http://127.0.0.1:* ws://127.0.0.1:*; frame-ancestors 'none'; base-uri 'none'; form-action 'none';\">";

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SecurityError {
    #[error("destination host `{0}` is not permitted by rustyBolt security policy")]
    DisallowedHost(String),

    #[error("insecure scheme `{0}` is not permitted for remote host")]
    InsecureScheme(String),

    #[error("disallowed port `{0}` for remote host")]
    DisallowedPort(u16),

    #[error("embedded user credentials in destination URL are forbidden")]
    InvalidUserInfo,

    #[error("malformed URL: {0}")]
    MalformedUrl(String),
}

pub fn is_allowed_remote_host(host: &str) -> bool {
    let lower = host.trim().to_ascii_lowercase();
    lower == ALLOWED_JAGEX_HOST || lower == ALLOWED_AUTH_HOST || lower == ALLOWED_REDIRECT_HOST
}

pub fn is_allowed_host(host: &str) -> bool {
    let lower = host.trim().to_ascii_lowercase();
    is_allowed_remote_host(&lower)
        || lower == "127.0.0.1"
        || lower == "localhost"
        || lower == "::1"
        || lower == "[::1]"
}

pub fn is_allowed_navigation(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("http://127.0.0.1:")
        || trimmed.starts_with("https://127.0.0.1:")
        || trimmed.starts_with("http://localhost:")
        || trimmed.starts_with("http://[::1]:")
}

pub fn is_allowed_external_url(url: &str) -> bool {
    if validate_url(url).is_err() {
        return false;
    }
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://account.jagex.com/")
        || lower.starts_with("https://auth.jagex.com/")
        || lower.starts_with("https://secure.runescape.com/")
}

pub fn csp_header_value() -> &'static str {
    CSP_VALUE
}

pub fn csp_meta_tag() -> &'static str {
    CSP_META_TAG
}

pub fn validate_url(raw_url: &str) -> Result<(), SecurityError> {
    let (scheme, remainder) = match raw_url.find("://") {
        Some(idx) => (&raw_url[..idx], &raw_url[idx + 3..]),
        None => {
            return Err(SecurityError::MalformedUrl(
                "missing scheme separator".to_string(),
            ))
        }
    };

    let scheme_lower = scheme.to_ascii_lowercase();

    let authority = match remainder.find(['/', '?', '#']) {
        Some(idx) => &remainder[..idx],
        None => remainder,
    };

    if authority.is_empty() {
        return Err(SecurityError::MalformedUrl("empty authority".to_string()));
    }

    if authority.contains('@') {
        return Err(SecurityError::InvalidUserInfo);
    }

    let (host, port_str) = if authority.starts_with('[') {
        match authority.find(']') {
            Some(close_bracket) => {
                let host_part = &authority[1..close_bracket];
                let after = &authority[close_bracket + 1..];
                let port = after.strip_prefix(':');
                (host_part, port)
            }
            None => {
                return Err(SecurityError::MalformedUrl(
                    "unclosed IPv6 bracket".to_string(),
                ))
            }
        }
    } else {
        match authority.find(':') {
            Some(idx) => (&authority[..idx], Some(&authority[idx + 1..])),
            None => (authority, None),
        }
    };

    let host_lower = host.to_ascii_lowercase();
    let port = match port_str {
        Some(p) => match p.parse::<u16>() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                return Err(SecurityError::MalformedUrl(
                    "invalid port number".to_string(),
                ))
            }
        },
        None => None,
    };

    if is_allowed_remote_host(&host_lower) {
        if scheme_lower != "https" {
            return Err(SecurityError::InsecureScheme(scheme.to_string()));
        }
        if let Some(p) = port {
            if p != 443 {
                return Err(SecurityError::DisallowedPort(p));
            }
        }
        return Ok(());
    }

    if host_lower == "127.0.0.1"
        || host_lower == "localhost"
        || host_lower == "::1"
        || host_lower == "[::1]"
    {
        if scheme_lower != "http" && scheme_lower != "https" && scheme_lower != "ws" {
            return Err(SecurityError::InsecureScheme(scheme.to_string()));
        }
        return Ok(());
    }

    Err(SecurityError::DisallowedHost(host.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_jagex_urls() {
        assert!(validate_url("https://account.jagex.com/oauth2/token").is_ok());
        assert!(validate_url("https://account.jagex.com/game-sessions").is_ok());
        assert!(validate_url("https://account.jagex.com/users/@me").is_ok());
        assert!(validate_url("https://account.jagex.com:443/oauth2/token").is_ok());
        assert!(validate_url("https://ACCOUNT.JAGEX.COM/users/@me").is_ok());
        assert!(validate_url("https://auth.jagex.com/game-session/v1/accounts").is_ok());
        assert!(validate_url("https://auth.jagex.com/game-session/v1/sessions").is_ok());
        assert!(validate_url("https://auth.jagex.com:443/game-session/v1/accounts").is_ok());
        assert!(validate_url("https://secure.runescape.com/m=weblogin/launcher-redirect").is_ok());
    }

    #[test]
    fn test_valid_localhost_urls() {
        assert!(validate_url("http://127.0.0.1:8080/#play").is_ok());
        assert!(validate_url("http://localhost:60649/api/launch").is_ok());
        assert!(validate_url("http://127.0.0.1:12345/api/wifi").is_ok());
        assert!(validate_url("http://[::1]:8080/api").is_ok());
    }

    #[test]
    fn test_rejected_external_hosts() {
        assert_eq!(
            validate_url("https://google.com/track"),
            Err(SecurityError::DisallowedHost("google.com".to_string()))
        );
        assert_eq!(
            validate_url("https://analytics.google.com/collect"),
            Err(SecurityError::DisallowedHost(
                "analytics.google.com".to_string()
            ))
        );
        assert_eq!(
            validate_url("https://evil.org/exfiltrate"),
            Err(SecurityError::DisallowedHost("evil.org".to_string()))
        );
    }

    #[test]
    fn test_rejected_subdomain_spoofing() {
        assert_eq!(
            validate_url("https://account.jagex.com.attacker.com/oauth2/token"),
            Err(SecurityError::DisallowedHost(
                "account.jagex.com.attacker.com".to_string()
            ))
        );
        assert_eq!(
            validate_url("https://fake-account.jagex.com/token"),
            Err(SecurityError::DisallowedHost(
                "fake-account.jagex.com".to_string()
            ))
        );
    }

    #[test]
    fn test_rejected_insecure_scheme_for_jagex() {
        assert_eq!(
            validate_url("http://account.jagex.com/oauth2/token"),
            Err(SecurityError::InsecureScheme("http".to_string()))
        );
    }

    #[test]
    fn test_rejected_port_for_jagex() {
        assert_eq!(
            validate_url("https://account.jagex.com:8443/oauth2/token"),
            Err(SecurityError::DisallowedPort(8443))
        );
    }

    #[test]
    fn test_rejected_userinfo() {
        assert_eq!(
            validate_url("https://user:pass@account.jagex.com/oauth2/token"),
            Err(SecurityError::InvalidUserInfo)
        );
    }

    #[test]
    fn test_navigation_filter() {
        assert!(is_allowed_navigation("http://127.0.0.1:60649/#play"));
        assert!(is_allowed_navigation("http://127.0.0.1:60649/#settings"));
        assert!(is_allowed_navigation("http://localhost:8080/"));
        assert!(!is_allowed_navigation("https://google.com"));
        assert!(!is_allowed_navigation("https://account.jagex.com"));
        assert!(!is_allowed_navigation("javascript:alert(1)"));
        assert!(!is_allowed_navigation("data:text/html,test"));
    }

    #[test]
    fn test_csp_content() {
        assert!(csp_header_value().contains("default-src 'none'"));
        assert!(csp_header_value().contains("connect-src http://127.0.0.1:*"));
        assert!(csp_meta_tag().contains("Content-Security-Policy"));
    }

    #[test]
    fn test_external_url_filter() {
        assert!(is_allowed_external_url(
            "https://account.jagex.com/oauth2/auth?response_type=code"
        ));
        assert!(is_allowed_external_url(
            "https://auth.jagex.com/game-session/v1/accounts"
        ));
        assert!(is_allowed_external_url(
            "https://secure.runescape.com/m=weblogin/launcher-redirect"
        ));
        assert!(!is_allowed_external_url("http://127.0.0.1:8080/"));
        assert!(!is_allowed_external_url("https://malicious.com/phish"));
        assert!(!is_allowed_external_url("https://google.com/"));
    }
}
