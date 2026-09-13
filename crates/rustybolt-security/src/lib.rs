//! Security policies, egress allowlists, and Content Security Policy enforcement.

use thiserror::Error;

pub const ALLOWED_JAGEX_HOST: &str = "account.jagex.com";
pub const ALLOWED_AUTH_HOST: &str = "auth.jagex.com";
pub const ALLOWED_REDIRECT_HOST: &str = "secure.runescape.com";
/// The hosts of the release check and download: the GitHub API, the release
/// page, and the two storage hosts that an asset download redirects to.
pub const ALLOWED_UPDATE_HOSTS: [&str; 4] = [
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
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
    lower == ALLOWED_JAGEX_HOST
        || lower == ALLOWED_AUTH_HOST
        || lower == ALLOWED_REDIRECT_HOST
        || ALLOWED_UPDATE_HOSTS.contains(&lower.as_str())
}

pub fn is_allowed_host(host: &str) -> bool {
    let lower = host.trim().to_ascii_lowercase();
    is_allowed_remote_host(&lower) || is_local_host(&lower)
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

/// Reads `url` and rejects one that carries user info.
fn parse_url(url: &str) -> Result<url::Url, SecurityError> {
    let parsed = url::Url::parse(url.trim())
        .map_err(|error| SecurityError::MalformedUrl(error.to_string()))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(SecurityError::InvalidUserInfo);
    }
    Ok(parsed)
}

fn is_local_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

/// Reports whether `url` is one of the two OAuth redirect targets that the
/// login window must intercept instead of load.
pub fn is_login_redirect(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("jagex:")
        || lower.starts_with("https://secure.runescape.com/m=weblogin/launcher-redirect")
        || lower.starts_with("http://localhost/")
        || lower.starts_with("http://localhost#")
        || lower == "http://localhost"
}

/// Reports whether the dedicated login window may load `url`.
///
/// Only HTTPS pages on Jagex-owned domains are allowed. The two redirect
/// targets are excluded here because [`is_login_redirect`] handles them.
pub fn is_allowed_login_navigation(url: &str) -> bool {
    let trimmed = url.trim();
    // Turnstile and similar widgets build their UI in about:blank/about:srcdoc
    // iframes; these carry no remote content of their own.
    if trimmed == "about:blank" || trimmed == "about:srcdoc" {
        return true;
    }
    if !trimmed
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
    {
        return false;
    }
    let Some(host) = parse_url(trimmed)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    // Jagex fronts its login with Cloudflare Turnstile; the challenge widget
    // is an iframe on challenges.cloudflare.com.
    const DOMAINS: [&str; 2] = ["jagex.com", "runescape.com"];
    const HOSTS: [&str; 1] = ["challenges.cloudflare.com"];
    HOSTS.contains(&host.as_str())
        || DOMAINS
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

pub fn csp_header_value() -> &'static str {
    CSP_VALUE
}

pub fn csp_meta_tag() -> &'static str {
    CSP_META_TAG
}

pub fn validate_url(raw_url: &str) -> Result<(), SecurityError> {
    let parsed = parse_url(raw_url)?;
    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| SecurityError::MalformedUrl("empty authority".to_string()))?
        .to_ascii_lowercase();

    if is_allowed_remote_host(&host) {
        if scheme != "https" {
            return Err(SecurityError::InsecureScheme(scheme.to_string()));
        }
        // `port()` is `None` for the default port of the scheme.
        if let Some(port) = parsed.port() {
            return Err(SecurityError::DisallowedPort(port));
        }
        return Ok(());
    }

    if is_local_host(&host) {
        if scheme != "http" && scheme != "https" && scheme != "ws" {
            return Err(SecurityError::InsecureScheme(scheme.to_string()));
        }
        return Ok(());
    }

    Err(SecurityError::DisallowedHost(host))
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
    fn test_update_hosts() {
        assert!(
            validate_url("https://api.github.com/repos/nullparity/rustyBolt/releases/latest")
                .is_ok()
        );
        assert!(validate_url(
            "https://github.com/nullparity/rustyBolt/releases/download/v1.0.0/a.tar.gz"
        )
        .is_ok());
        assert!(validate_url("https://objects.githubusercontent.com/x").is_ok());
        assert!(validate_url("http://api.github.com/x").is_err());
        assert!(validate_url("https://raw.githubusercontent.com/x").is_err());
        assert!(!is_allowed_external_url(
            "https://github.com/nullparity/rustyBolt"
        ));
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
    fn test_login_window_policy() {
        assert!(is_allowed_login_navigation(
            "https://account.jagex.com/oauth2/auth?x=1"
        ));
        assert!(is_allowed_login_navigation("https://auth.jagex.com/"));
        assert!(is_allowed_login_navigation("https://www.runescape.com/"));
        assert!(is_allowed_login_navigation("HTTPS://Account.Jagex.com/"));
        assert!(is_allowed_login_navigation(
            "https://challenges.cloudflare.com/cdn-cgi/challenge-platform/"
        ));
        assert!(!is_allowed_login_navigation("https://cloudflare.com/"));
        assert!(is_allowed_login_navigation("about:blank"));
        assert!(is_allowed_login_navigation("about:srcdoc"));
        assert!(!is_allowed_login_navigation("about:config"));
        assert!(!is_allowed_login_navigation("http://account.jagex.com/"));
        assert!(!is_allowed_login_navigation("https://jagex.com.evil.com/"));
        assert!(!is_allowed_login_navigation("https://evil.com/?jagex.com"));
        assert!(!is_allowed_login_navigation(
            "https://evil.com#account.jagex.com"
        ));
        assert!(!is_allowed_login_navigation("http://127.0.0.1:8080/"));

        assert!(is_login_redirect(
            "https://secure.runescape.com/m=weblogin/launcher-redirect?code=abc&state=x"
        ));
        assert!(is_login_redirect(
            "http://localhost/#code=a&id_token=b&state=c"
        ));
        assert!(is_login_redirect("http://localhost#code=a"));
        assert!(is_login_redirect("jagex:code=a,state=b,intent=social_auth"));
        assert!(!is_login_redirect("https://account.jagex.com/"));
        assert!(!is_login_redirect("http://localhost.evil.com/"));
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
