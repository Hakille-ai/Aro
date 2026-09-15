# ARO Self-Host

ARO self-host runs the Rust API, PostgreSQL, Redis, a migration job, a worker process, and a TLS reverse proxy. The desktop app connects to the API over HTTPS and keeps local model, voice, and provider-key material on the device.

## Docker Compose

1. Build or pull an API image:

```powershell
docker build -t aro-api:latest .
```

2. Create production secrets outside Git:

```powershell
New-Item -ItemType Directory -Force deploy\secrets
Set-Content deploy\secrets\postgres_user aro_migrator
Set-Content deploy\secrets\postgres_password "<random database password>"
Set-Content deploy\secrets\aro_app_password "<independent random application password>"
Set-Content deploy\secrets\aro_worker_password "<independent random worker password>"
Copy-Item .env.production.example .env.production
Copy-Item .env.production.api.example .env.production.api
Copy-Item .env.production.worker.example .env.production.worker
Copy-Item .env.production.migrate.example .env.production.migrate
```

Then edit the four environment files:

- Keep only non-secret shared configuration in `.env.production`, including the explicit HTTPS
  `ARO_PUBLIC_BASE_URL`, `ARO_CORS_ALLOWED_ORIGINS`, and digest-pinned image references.
- Put only `DATABASE_URL`, `ARO_JWT_SECRET`, `ARO_SECRETS_KEY`, `ARO_METRICS_TOKEN`, and the API S3
  identity in `.env.production.api`.
- Put only `ARO_WORKER_DATABASE_URL` and the scanner S3 identity in `.env.production.worker`.
- Put only the schema-owner `ARO_MIGRATION_DATABASE_URL` in `.env.production.migrate`.
- Keep all three database URLs consistent with the migrator/app/worker secret files.
- Keep `ARO_SIGNUP_MODE=closed` for enterprise installs and create the first owner with the admin command.

4. Start the stack:

```powershell
$env:ARO_DOMAIN="aro.example.com"
$env:ARO_ADMIN_EMAIL="admin@example.com"
docker compose --env-file .env.production -f deploy\compose.prod.yml up -d
```

5. Create the first owner:

```powershell
docker compose --env-file .env.production -f deploy\compose.prod.yml run --rm api admin create-owner --email owner@example.com --password "<strong password>" --name "Owner" --organization "Example"
```

6. Verify:

```powershell
Invoke-RestMethod https://aro.example.com/live
Invoke-RestMethod https://aro.example.com/ready
powershell -ExecutionPolicy Bypass -File scripts\smoke-api.ps1 -BaseUrl https://aro.example.com -SkipDockerDatabaseChecks
```

## Kubernetes

Package and pin the API image, then provision secrets through External Secrets, Sealed Secrets,
SOPS, or an equivalent cluster secret manager. The chart does not create weak credentials by
default. `aro-secrets` must contain:

- `DATABASE_URL` for the fixed `aro_app` runtime role;
- `ARO_WORKER_DATABASE_URL` for the fixed `aro_worker` role;
- `ARO_MIGRATION_DATABASE_URL` for the schema owner/migrator;
- independent random values for `ARO_JWT_SECRET`, `ARO_SECRETS_KEY`, and `ARO_METRICS_TOKEN`;
- `ARO_REDIS_URL` when the bundled Redis deployment is disabled.

When the bundled PostgreSQL StatefulSet is enabled, also provision `aro-postgres` with
`POSTGRES_PASSWORD`, `ARO_POSTGRES_APP_PASSWORD`, and `ARO_POSTGRES_WORKER_PASSWORD`. The URLs in
`aro-secrets` must use those same credentials. Managed PostgreSQL and Redis are recommended for
availability, backup, encryption, and failover requirements.

Install the chart with explicit public origins and immutable image digests:

```powershell
helm upgrade --install aro deploy\helm\aro --wait --atomic `
  --set image.repository=ghcr.io/example/aro-api `
  --set image.digest=sha256:<published-image-digest> `
  --set ingress.enabled=true `
  --set ingress.host=aro.example.com `
  --set-string api.env.ARO_PUBLIC_BASE_URL=https://aro.example.com `
  --set-string api.env.ARO_CORS_ALLOWED_ORIGINS=https://aro.example.com
```

Each chart revision creates a bounded migration Job. API startup remains fail-closed until the
schema is current; `--wait --atomic` makes a failed migration fail and roll back the Helm release.
The bundled PostgreSQL, Redis, and ClamAV workloads are single-instance conveniences, not a
high-availability data plane. Pin a digest for every enabled image before production rollout.

## Desktop Clients

Deploy the Tauri desktop app through MDM/SCCM/Intune or a signed installer. Set:

```powershell
ARO_API_BASE_URL=https://aro.example.com
```

Remote API URLs must use HTTPS. Local model paths, voice runtimes, and provider API keys remain device-local.
