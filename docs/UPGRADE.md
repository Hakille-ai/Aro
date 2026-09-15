# Upgrade Runbook

ARO upgrades use an expand/contract discipline: deploy additive migrations first, roll the API forward, then remove old compatibility only in a later release.

## Compose Upgrade

1. Back up PostgreSQL and secrets.
2. Pull or build the new image.
3. Run migrations:

```powershell
docker compose -f deploy\compose.prod.yml run --rm migrate
```

4. Roll the API and worker:

```powershell
docker compose -f deploy\compose.prod.yml up -d api worker
```

5. Verify `/ready`, `/metrics`, and `scripts\smoke-api.ps1`.

## Kubernetes Upgrade

```powershell
helm upgrade aro deploy\helm\aro --reuse-values --set image.tag=vX.Y.Z
```

The chart runs the migration job before the Deployment rolls. If the migration fails, the API rollout does not proceed.

## Rollback

Rollback the image only if the migration is backward compatible:

```powershell
docker compose -f deploy\compose.prod.yml up -d api worker
```

For non-compatible database failures, restore the latest backup into a clean database and restart with the previous image and matching secrets.
