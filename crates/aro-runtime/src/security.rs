use aro_core::{AroError, AroResult};
use url::Url;

pub fn ensure_loopback_url(raw_url: &str) -> AroResult<Url> {
    let url = Url::parse(raw_url)
        .map_err(|err| AroError::Configuration(format!("invalid endpoint URL: {err}")))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AroError::Security(format!(
                "unsupported endpoint scheme '{other}'"
            )))
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| AroError::Security("endpoint has no host".to_string()))?;

    let allowed = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if !allowed {
        return Err(AroError::Security(format!(
            "endpoint must be local loopback, got {host}"
        )));
    }

    Ok(url)
}

pub fn ensure_remote_https_url(raw_url: &str) -> AroResult<Url> {
    let url = Url::parse(raw_url)
        .map_err(|err| AroError::Configuration(format!("invalid endpoint URL: {err}")))?;

    if url.scheme() != "https" {
        return Err(AroError::Security(
            "remote provider endpoint must use HTTPS".to_string(),
        ));
    }
    if url.host_str().is_none() {
        return Err(AroError::Security("endpoint has no host".to_string()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AroError::Security(
            "provider endpoint must not include credentials".to_string(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AroError::Security(
            "provider endpoint must not include query or fragment".to_string(),
        ));
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback() {
        assert!(ensure_loopback_url("http://127.0.0.1:11434").is_ok());
        assert!(ensure_loopback_url("http://localhost:8080").is_ok());
    }

    #[test]
    fn rejects_remote_hosts() {
        assert!(ensure_loopback_url("https://api.example.com").is_err());
    }

    #[test]
    fn remote_providers_require_clean_https() {
        assert!(ensure_remote_https_url("https://api.example.com/v1").is_ok());
        assert!(ensure_remote_https_url("http://api.example.com/v1").is_err());
        assert!(ensure_remote_https_url("https://user:pass@api.example.com/v1").is_err());
        assert!(ensure_remote_https_url("https://api.example.com/v1?key=secret").is_err());
    }
}
