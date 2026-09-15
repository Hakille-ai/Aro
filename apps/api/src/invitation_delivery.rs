use std::{env, time::Duration};

use aro_core::{AroError, AroResult, MembershipRole};
use aro_store::ClaimedInvitationDelivery;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use url::{Host, Url};

#[derive(Clone)]
pub struct InvitationDeliveryService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    message_id_domain: String,
    acceptance_base_url: Url,
    timeout_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationDeliveryFailure {
    Transient(&'static str),
    Permanent(&'static str),
}

impl InvitationDeliveryFailure {
    pub fn safe_code(self) -> &'static str {
        match self {
            Self::Transient(code) | Self::Permanent(code) => code,
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, Self::Transient(_))
    }
}

impl InvitationDeliveryService {
    pub fn enabled_from_env() -> AroResult<bool> {
        env_bool("ARO_INVITATION_DELIVERY_ENABLED", false)
    }

    pub fn from_env(environment: &str, enabled: bool) -> AroResult<Option<Self>> {
        if !enabled {
            return Ok(None);
        }

        let relay = required_env("ARO_SMTP_RELAY")?;
        validate_relay_host(&relay)?;
        let tls_mode = optional_env("ARO_SMTP_TLS_MODE")?
            .unwrap_or_else(|| "starttls".to_string())
            .trim()
            .to_ascii_lowercase();
        if !matches!(tls_mode.as_str(), "starttls" | "tls") {
            return Err(AroError::Configuration(
                "ARO_SMTP_TLS_MODE must be starttls or tls".into(),
            ));
        }
        let port = optional_env("ARO_SMTP_PORT")?
            .map(|value| {
                value.parse::<u16>().map_err(|_| {
                    AroError::Configuration("ARO_SMTP_PORT must be between 1 and 65535".into())
                })
            })
            .transpose()?
            .unwrap_or(if tls_mode == "tls" { 465 } else { 587 });
        if port == 0 {
            return Err(AroError::Configuration(
                "ARO_SMTP_PORT must be between 1 and 65535".into(),
            ));
        }
        let timeout_seconds = optional_env("ARO_SMTP_TIMEOUT_SECONDS")?
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    AroError::Configuration(
                        "ARO_SMTP_TIMEOUT_SECONDS must be an integer between 5 and 120".into(),
                    )
                })
            })
            .transpose()?
            .unwrap_or(30);
        if !(5..=120).contains(&timeout_seconds) {
            return Err(AroError::Configuration(
                "ARO_SMTP_TIMEOUT_SECONDS must be between 5 and 120".into(),
            ));
        }

        let username = optional_env("ARO_SMTP_USERNAME")?;
        let password = optional_env("ARO_SMTP_PASSWORD")?;
        if username.is_some() != password.is_some() {
            return Err(AroError::Configuration(
                "ARO_SMTP_USERNAME and ARO_SMTP_PASSWORD must be configured together".into(),
            ));
        }
        let allow_unauthenticated = env_bool("ARO_SMTP_ALLOW_UNAUTHENTICATED", false)?;
        if environment == "production" && username.is_none() && !allow_unauthenticated {
            return Err(AroError::Configuration(
                "authenticated SMTP is required in production unless ARO_SMTP_ALLOW_UNAUTHENTICATED=true is explicitly set for a trusted private relay".into(),
            ));
        }

        let builder = if tls_mode == "starttls" {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&relay)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&relay)
        };
        let mut builder = builder
            .map_err(|_| AroError::Configuration("ARO_SMTP_RELAY is invalid".into()))?
            .port(port)
            .timeout(Some(Duration::from_secs(timeout_seconds)));
        if let (Some(username), Some(password)) = (username, password) {
            builder = builder.credentials(Credentials::new(username, password));
        }

        let from: Mailbox = required_env("ARO_SMTP_FROM")?
            .parse()
            .map_err(|_| AroError::Configuration("ARO_SMTP_FROM is invalid".into()))?;
        let from_address = from.email.to_string();
        let message_id_domain = from_address
            .rsplit_once('@')
            .map(|(_, domain)| domain.to_ascii_lowercase())
            .filter(|domain| {
                !domain.is_empty()
                    && !domain
                        .chars()
                        .any(|character| matches!(character, '<' | '>' | '@'))
            })
            .ok_or_else(|| AroError::Configuration("ARO_SMTP_FROM domain is invalid".into()))?;

        let acceptance_base_url = validate_acceptance_base_url(
            &required_env("ARO_INVITATION_ACCEPT_BASE_URL")?,
            environment == "production",
        )?;

        Ok(Some(Self {
            mailer: builder.build(),
            from,
            message_id_domain,
            acceptance_base_url,
            timeout_seconds,
        }))
    }

    pub fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }

    pub async fn send(
        &self,
        delivery: &ClaimedInvitationDelivery,
    ) -> Result<(), InvitationDeliveryFailure> {
        let message = self.build_message(delivery)?;

        match tokio::time::timeout(
            Duration::from_secs(self.timeout_seconds),
            self.mailer.send(message),
        )
        .await
        {
            Err(_) => Err(InvitationDeliveryFailure::Transient("smtp_timeout")),
            Ok(result) => result.map(|_| ()).map_err(|error| {
                if error.is_permanent() {
                    InvitationDeliveryFailure::Permanent("smtp_permanent_rejection")
                } else if error.is_timeout() {
                    InvitationDeliveryFailure::Transient("smtp_timeout")
                } else if error.is_tls() {
                    InvitationDeliveryFailure::Transient("smtp_tls_failure")
                } else if error.is_transient() {
                    InvitationDeliveryFailure::Transient("smtp_transient_rejection")
                } else {
                    InvitationDeliveryFailure::Transient("smtp_transport_failure")
                }
            }),
        }
    }

    fn build_message(
        &self,
        delivery: &ClaimedInvitationDelivery,
    ) -> Result<Message, InvitationDeliveryFailure> {
        let recipient_name = delivery.name.trim();
        let organization_name = delivery.organization_name.trim();
        if !valid_message_label(recipient_name) || !valid_message_label(organization_name) {
            return Err(InvitationDeliveryFailure::Permanent(
                "invalid_invitation_metadata",
            ));
        }
        let recipient_address = delivery
            .email
            .parse()
            .map_err(|_| InvitationDeliveryFailure::Permanent("invalid_recipient"))?;
        let recipient = Mailbox::new(Some(recipient_name.to_string()), recipient_address);
        let acceptance_url = acceptance_url(&self.acceptance_base_url, &delivery.token)
            .map_err(|_| InvitationDeliveryFailure::Permanent("invalid_acceptance_url"))?;
        let body = format!(
            "Hello {},\n\nYou have been invited to join {} on ARO as {}.\n\nAccept the invitation:\n{}\n\nThis single-use link expires at {}. If you were not expecting this invitation, you can ignore this message. Do not forward the link.\n",
            recipient_name,
            organization_name,
            role_label(&delivery.role),
            acceptance_url,
            delivery.expires_at.to_rfc3339(),
        );
        Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(format!("Invitation to join {organization_name} on ARO"))
            .message_id(Some(format!(
                "<aro-invitation-{}@{}>",
                delivery.id, self.message_id_domain
            )))
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|_| InvitationDeliveryFailure::Permanent("message_build_failed"))
    }
}

fn valid_message_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 800
        && value.chars().count() <= 200
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
}

fn role_label(role: &MembershipRole) -> &'static str {
    match role {
        MembershipRole::Owner => "owner",
        MembershipRole::Admin => "admin",
        MembershipRole::Manager => "manager",
        MembershipRole::Member => "member",
        MembershipRole::Guest => "guest",
    }
}

fn acceptance_url(base_url: &Url, token: &str) -> AroResult<Url> {
    if !(32..=512).contains(&token.len())
        || !token.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AroError::Security("invalid invitation token".into()));
    }
    let fragment = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", token)
        .finish();
    let mut url = base_url.clone();
    url.set_fragment(Some(&fragment));
    Ok(url)
}

fn validate_acceptance_base_url(raw: &str, production: bool) -> AroResult<Url> {
    let url = Url::parse(raw.trim())
        .map_err(|_| AroError::Configuration("ARO_INVITATION_ACCEPT_BASE_URL is invalid".into()))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.port() == Some(0)
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AroError::Configuration(
            "ARO_INVITATION_ACCEPT_BASE_URL must not contain credentials, query, or fragment"
                .into(),
        ));
    }
    match url.scheme() {
        "https" => {}
        "http" if !production && is_loopback_url(&url) => {}
        _ => {
            return Err(AroError::Configuration(
                "ARO_INVITATION_ACCEPT_BASE_URL must use HTTPS (HTTP loopback is allowed only outside production)".into(),
            ));
        }
    }
    Ok(url)
}

fn validate_relay_host(relay: &str) -> AroResult<()> {
    let relay = relay.trim();
    let hostname = relay.strip_suffix('.').unwrap_or(relay);
    let valid = !hostname.is_empty()
        && relay.len() <= 253
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(AroError::Configuration(
            "ARO_SMTP_RELAY must be a hostname without scheme, credentials, path, or port".into(),
        ))
    }
}

fn required_env(name: &str) -> AroResult<String> {
    optional_env(name)?.ok_or_else(|| AroError::Configuration(format!("{name} is required")))
}

fn optional_env(name: &str) -> AroResult<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value.trim().to_string()).filter(|value| !value.is_empty())),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(AroError::Configuration(format!(
            "{name} must contain valid Unicode"
        ))),
    }
}

fn env_bool(name: &str, default: bool) -> AroResult<bool> {
    match env::var(name) {
        Ok(value) => parse_bool(&value).ok_or_else(|| {
            AroError::Configuration(format!(
                "{name} must be one of: true, false, 1, 0, yes, no, on, off"
            ))
        }),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => Err(AroError::Configuration(format!(
            "{name} must contain valid Unicode"
        ))),
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn is_loopback_url(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_service() -> InvitationDeliveryService {
        InvitationDeliveryService {
            mailer: AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("smtp.example.com")
                .expect("valid relay")
                .build(),
            from: "ARO <noreply@aro.example>".parse().expect("valid sender"),
            message_id_domain: "aro.example".to_string(),
            acceptance_base_url: validate_acceptance_base_url(
                "https://app.aro.example/invitations/accept",
                true,
            )
            .expect("valid acceptance URL"),
            timeout_seconds: 30,
        }
    }

    fn claimed_delivery(token: &str) -> ClaimedInvitationDelivery {
        ClaimedInvitationDelivery {
            id: Uuid::parse_str("a56d4b8c-9bcb-4fab-a72f-865f44e8e488")
                .expect("valid invitation id"),
            organization_id: Uuid::new_v4(),
            organization_name: "Example Organization".to_string(),
            email: "invitee@example.com".to_string(),
            name: "Invited Person".to_string(),
            role: MembershipRole::Manager,
            token: token.to_string(),
            expires_at: Utc::now(),
            attempts: 1,
            lease_token: Uuid::new_v4(),
        }
    }

    #[test]
    fn acceptance_tokens_stay_in_the_url_fragment() {
        let base = validate_acceptance_base_url("https://app.aro.example/invite", true)
            .expect("valid base URL");
        let token = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        let url = acceptance_url(&base, token).expect("acceptance URL");
        let expected_fragment = format!("token={token}");
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), Some(expected_fragment.as_str()));
        assert!(!url
            .as_str()
            .split('#')
            .next()
            .unwrap_or_default()
            .contains(token));
    }

    #[test]
    fn acceptance_base_url_is_https_or_local_loopback() {
        assert!(validate_acceptance_base_url("https://app.aro.example/invite", true).is_ok());
        assert_eq!(
            validate_acceptance_base_url("https://app.aro.example/invite", true)
                .expect("valid path")
                .path(),
            "/invite"
        );
        assert!(validate_acceptance_base_url("http://127.0.0.1:1420", false).is_ok());
        assert!(validate_acceptance_base_url("http://[::1]:1420", false).is_ok());
        assert!(validate_acceptance_base_url("http://example.com/invite", false).is_err());
        assert!(validate_acceptance_base_url("http://127.0.0.1:1420", true).is_err());
        assert!(validate_acceptance_base_url("https://user@example.com/invite", true).is_err());
        assert!(validate_acceptance_base_url("https://app.aro.example:0/invite", true).is_err());
    }

    #[test]
    fn relay_configuration_rejects_urls_and_inline_credentials() {
        assert!(validate_relay_host("smtp.example.com").is_ok());
        assert!(validate_relay_host("internal-relay").is_ok());
        assert!(validate_relay_host("https://smtp.example.com").is_err());
        assert!(validate_relay_host("user@smtp.example.com").is_err());
        assert!(validate_relay_host("smtp.example.com:587").is_err());
        assert!(validate_relay_host("smtp..example.com").is_err());
        assert!(validate_relay_host("-smtp.example.com").is_err());
        assert!(validate_relay_host("smtp.example.com?mode=unsafe").is_err());
    }

    #[test]
    fn smtp_boolean_configuration_is_strict() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool(" OFF "), Some(false));
        assert_eq!(parse_bool("enabled"), None);
        assert_eq!(parse_bool(""), None);
    }

    #[tokio::test]
    async fn message_id_is_stable_and_bearer_token_never_enters_headers() {
        let token = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        let delivery = claimed_delivery(token);
        let formatted = String::from_utf8(
            test_service()
                .build_message(&delivery)
                .expect("message should build")
                .formatted(),
        )
        .expect("email is UTF-8");
        let (headers, body) = formatted
            .split_once("\r\n\r\n")
            .expect("email header/body boundary");

        assert!(headers.contains(
            "Message-ID: <aro-invitation-a56d4b8c-9bcb-4fab-a72f-865f44e8e488@aro.example>"
        ));
        assert!(!headers.contains(token));
        let unfolded_body = body.replace("=\r\n", "");
        assert_eq!(unfolded_body.matches(token).count(), 1);
        assert!(unfolded_body.contains("#token"));
    }

    #[tokio::test]
    async fn message_headers_resist_invitee_and_tenant_header_injection() {
        let token = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
        let mut delivery = claimed_delivery(token);
        delivery.name = "Person\r\nBcc: attacker@example.com".to_string();
        delivery.organization_name = "Tenant\r\nX-Injected: true".to_string();
        assert_eq!(
            test_service().build_message(&delivery).err(),
            Some(InvitationDeliveryFailure::Permanent(
                "invalid_invitation_metadata"
            ))
        );
    }

    #[test]
    fn acceptance_tokens_are_length_bounded() {
        let base = validate_acceptance_base_url("https://app.aro.example/invite", true)
            .expect("valid base URL");
        assert!(acceptance_url(&base, &"a".repeat(31)).is_err());
        assert!(acceptance_url(&base, &"a".repeat(513)).is_err());
    }
}
