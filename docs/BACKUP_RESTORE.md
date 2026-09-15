# Backup And Restore

PostgreSQL is the durable source of truth. `ARO_SECRETS_KEY` is required to decrypt encrypted plugin, MCP, hook, integration, and client-state material. A database backup without the matching secret key is incomplete.

## Backup

For Compose:

```powershell
docker compose -f deploy\compose.prod.yml exec postgres pg_dump -U aro -d aro -Fc -f /backups/aro-$(Get-Date -Format yyyyMMddHHmmss).dump
```

Also back up:

- `.env.production` (shared non-secret configuration)
- `.env.production.api`, `.env.production.worker`, and `.env.production.migrate` (restore from the external secret manager, never from the database backup)
- `deploy/secrets/postgres_user`
- `deploy/secrets/postgres_password`
- the active `ARO_SECRETS_KEY`
- the active `ARO_JWT_SECRET` if you need existing JWTs to remain valid during emergency restore

For managed PostgreSQL, enable PITR/WAL archiving and run a restore drill at least monthly.

## Restore Drill

1. Start a clean PostgreSQL instance.
2. Restore the dump:

```powershell
pg_restore --clean --if-exists -U aro -d aro aro.dump
```

3. Start `aro-api migrate`.
4. Start the API with the restored `ARO_SECRETS_KEY`.
5. Run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-api.ps1 -BaseUrl https://restored.example.com -SkipDockerDatabaseChecks
```

## RPO/RTO Defaults

- Beta SaaS: daily snapshot plus PITR, RPO <= 24h until usage requires tighter targets.
- Enterprise self-host: customer-owned policy, but ARO support should require a tested restore before production use.
