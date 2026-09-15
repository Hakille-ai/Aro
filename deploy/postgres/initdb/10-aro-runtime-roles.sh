#!/usr/bin/env bash
set -Eeuo pipefail

app_password="$(tr -d '\r\n' < /run/secrets/aro_app_password)"
worker_password="$(tr -d '\r\n' < /run/secrets/aro_worker_password)"

if [[ -z "${app_password}" || -z "${worker_password}" ]]; then
  echo "ARO runtime database role passwords must not be empty" >&2
  exit 1
fi

psql --username "${POSTGRES_USER}" --dbname "${POSTGRES_DB}" \
  --set=ON_ERROR_STOP=1 \
  --set=app_password="${app_password}" \
  --set=worker_password="${worker_password}" <<'SQL'
SELECT format(
  'CREATE ROLE aro_app LOGIN PASSWORD %L NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS',
  :'app_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app')
\gexec

SELECT format(
  'CREATE ROLE aro_worker LOGIN PASSWORD %L NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS',
  :'worker_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker')
\gexec

ALTER ROLE aro_app WITH
  PASSWORD :'app_password'
  NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
ALTER ROLE aro_worker WITH
  PASSWORD :'worker_password'
  NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
ALTER ROLE aro_app SET row_security = on;
ALTER ROLE aro_worker SET row_security = on;
SQL
