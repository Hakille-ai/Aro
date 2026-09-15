{{- define "aro.name" -}}
aro
{{- end -}}

{{- define "aro.fullname" -}}
{{- $base := printf "%s-%s" .Release.Name (include "aro.name" .) -}}
{{- if gt (len $base) 44 -}}
{{- printf "%s-%s" ($base | trunc 35 | trimSuffix "-") ($base | sha256sum | trunc 8) -}}
{{- else -}}
{{- $base -}}
{{- end -}}
{{- end -}}

{{- define "aro.migrationJobName" -}}
{{- $suffix := printf "-migrate-%d" (int .Release.Revision) -}}
{{- $prefixLength := sub 63 (len $suffix) | int -}}
{{- printf "%s%s" ((include "aro.fullname" .) | trunc $prefixLength | trimSuffix "-") $suffix -}}
{{- end -}}

{{- define "aro.image" -}}
{{- if .Values.image.digest -}}
{{ printf "%s@%s" .Values.image.repository .Values.image.digest }}
{{- else -}}
{{ printf "%s:%s" .Values.image.repository .Values.image.tag }}
{{- end -}}
{{- end -}}

{{- define "aro.dependencyImage" -}}
{{- $image := index . 0 -}}
{{- $digest := index . 1 -}}
{{- if $digest -}}
{{- printf "%s@%s" $image $digest -}}
{{- else -}}
{{- $image -}}
{{- end -}}
{{- end -}}

{{- define "aro.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{ default (include "aro.fullname" .) .Values.serviceAccount.name }}
{{- else -}}
{{ required "serviceAccount.name is required when serviceAccount.create=false" .Values.serviceAccount.name }}
{{- end -}}
{{- end -}}

{{- define "aro.secretName" -}}
{{- if .Values.secrets.create -}}
{{ include "aro.fullname" . }}-secret
{{- else -}}
{{ required "secrets.existingSecret is required when secrets.create=false" .Values.secrets.existingSecret }}
{{- end -}}
{{- end -}}

{{- define "aro.postgresSecretName" -}}
{{- if .Values.postgres.existingSecret -}}
{{- .Values.postgres.existingSecret -}}
{{- else -}}
{{- printf "%s-postgres" (include "aro.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "aro.apiDatabaseUrl" -}}
{{- if .Values.secrets.databaseUrl -}}
{{- .Values.secrets.databaseUrl -}}
{{- else if and .Values.postgres.enabled (not .Values.postgres.existingSecret) -}}
{{- printf "postgres://aro_app:%s@%s-postgres:5432/%s" (urlquery .Values.postgres.appPassword) (include "aro.fullname" .) (urlquery .Values.postgres.database) -}}
{{- else -}}
{{- required "secrets.databaseUrl is required when postgres.enabled=false or postgres.existingSecret is used with secrets.create=true" .Values.secrets.databaseUrl -}}
{{- end -}}
{{- end -}}

{{- define "aro.workerDatabaseUrl" -}}
{{- if .Values.secrets.workerDatabaseUrl -}}
{{- .Values.secrets.workerDatabaseUrl -}}
{{- else if and .Values.postgres.enabled (not .Values.postgres.existingSecret) -}}
{{- printf "postgres://aro_worker:%s@%s-postgres:5432/%s" (urlquery .Values.postgres.workerPassword) (include "aro.fullname" .) (urlquery .Values.postgres.database) -}}
{{- else -}}
{{- required "secrets.workerDatabaseUrl is required when postgres.enabled=false or postgres.existingSecret is used with secrets.create=true" .Values.secrets.workerDatabaseUrl -}}
{{- end -}}
{{- end -}}

{{- define "aro.migrationDatabaseUrl" -}}
{{- if .Values.secrets.migrationDatabaseUrl -}}
{{- .Values.secrets.migrationDatabaseUrl -}}
{{- else if and .Values.postgres.enabled (not .Values.postgres.existingSecret) -}}
{{- printf "postgres://%s:%s@%s-postgres:5432/%s" (urlquery .Values.postgres.user) (urlquery .Values.postgres.password) (include "aro.fullname" .) (urlquery .Values.postgres.database) -}}
{{- else -}}
{{- required "secrets.migrationDatabaseUrl is required when postgres.enabled=false or postgres.existingSecret is used with secrets.create=true" .Values.secrets.migrationDatabaseUrl -}}
{{- end -}}
{{- end -}}
