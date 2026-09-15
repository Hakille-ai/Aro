# Security Hardening

## Required For Public SaaS

- Keep `ARO_SIGNUP_MODE=closed` until email verification, reset password, anti-abuse, and rate limits are enabled for the target environment.
- Use explicit HTTPS CORS origins. Wildcards are rejected by the API.
- Use unique 64+ character `ARO_JWT_SECRET` and `ARO_SECRETS_KEY` per environment.
- Set `ARO_JWT_ISSUER`, `ARO_JWT_AUDIENCE`, and `ARO_JWT_KEY_ID`.
- Run API behind TLS with HSTS.
- Ship logs in JSON mode with redaction rules; do not log prompts, memory content, raw audio, transcripts, tokens, API keys, or database URLs.
- Scan images and dependencies before release.

## Required For Enterprise GA

- Use SSO/OIDC or SAML and disable open signup.
- Use a customer vault/KMS when available. Keep the local `ARO_SECRETS_KEY` provider only for small self-host installs.
- Export audit logs to SIEM.
- Restrict egress for integration workers and model providers.
- Run restore drills and upgrade rehearsals before production rollout.

## Database Isolation

ARO enforces tenancy in the application and adds composite tenant foreign keys for agent runtime data. PostgreSQL RLS should be introduced as the next hardening phase once the request path sets a trusted tenant context for every query.
