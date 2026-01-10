{{/*
Expand the name of the chart.
*/}}
{{- define "semantic-explorer.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "semantic-explorer.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "semantic-explorer.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "semantic-explorer.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "semantic-explorer.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
API labels
*/}}
{{- define "semantic-explorer.api.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.api.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: api
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
API selector labels
*/}}
{{- define "semantic-explorer.api.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-api
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: api
{{- end }}

{{/*
Worker Collections labels
*/}}
{{- define "semantic-explorer.workerCollections.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.workerCollections.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: worker-collections
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Worker Collections selector labels
*/}}
{{- define "semantic-explorer.workerCollections.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-worker-collections
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: worker-collections
{{- end }}

{{/*
Worker Datasets labels
*/}}
{{- define "semantic-explorer.workerDatasets.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.workerDatasets.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: worker-datasets
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Worker Datasets selector labels
*/}}
{{- define "semantic-explorer.workerDatasets.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-worker-datasets
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: worker-datasets
{{- end }}

{{/*
Worker Visualizations labels
*/}}
{{- define "semantic-explorer.workerVisualizations.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.workerVisualizations.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: worker-visualizations
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Worker Visualizations selector labels
*/}}
{{- define "semantic-explorer.workerVisualizations.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-worker-visualizations
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: worker-visualizations
{{- end }}

{{/*
Create the name of the service account to use for API
*/}}
{{- define "semantic-explorer.api.serviceAccountName" -}}
{{- if .Values.api.serviceAccount.create }}
{{- default (printf "%s-api" (include "semantic-explorer.fullname" .)) .Values.api.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.api.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name of the service account to use for Worker Collections
*/}}
{{- define "semantic-explorer.workerCollections.serviceAccountName" -}}
{{- if .Values.workerCollections.serviceAccount.create }}
{{- default (printf "%s-worker-collections" (include "semantic-explorer.fullname" .)) .Values.workerCollections.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.workerCollections.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name of the service account to use for Worker Datasets
*/}}
{{- define "semantic-explorer.workerDatasets.serviceAccountName" -}}
{{- if .Values.workerDatasets.serviceAccount.create }}
{{- default (printf "%s-worker-datasets" (include "semantic-explorer.fullname" .)) .Values.workerDatasets.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.workerDatasets.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name of the service account to use for Worker Visualizations
*/}}
{{- define "semantic-explorer.workerVisualizations.serviceAccountName" -}}
{{- if .Values.workerVisualizations.serviceAccount.create }}
{{- default (printf "%s-worker-visualizations" (include "semantic-explorer.fullname" .)) .Values.workerVisualizations.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.workerVisualizations.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Get the image registry
*/}}
{{- define "semantic-explorer.imageRegistry" -}}
{{- if .registry }}
{{- .registry }}
{{- else if .global.imageRegistry }}
{{- .global.imageRegistry }}
{{- end }}
{{- end }}

{{/*
Get the full image name for API
*/}}
{{- define "semantic-explorer.api.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.api.image.registry "global" .Values.global) }}
{{- $repository := .Values.api.image.repository }}
{{- $tag := .Values.api.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Get the full image name for Worker Collections
*/}}
{{- define "semantic-explorer.workerCollections.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.workerCollections.image.registry "global" .Values.global) }}
{{- $repository := .Values.workerCollections.image.repository }}
{{- $tag := .Values.workerCollections.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Get the full image name for Worker Datasets
*/}}
{{- define "semantic-explorer.workerDatasets.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.workerDatasets.image.registry "global" .Values.global) }}
{{- $repository := .Values.workerDatasets.image.repository }}
{{- $tag := .Values.workerDatasets.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Get the full image name for Worker Visualizations
*/}}
{{- define "semantic-explorer.workerVisualizations.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.workerVisualizations.image.registry "global" .Values.global) }}
{{- $repository := .Values.workerVisualizations.image.repository }}
{{- $tag := .Values.workerVisualizations.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Worker Visualizations Python labels
*/}}
{{- define "semantic-explorer.workerVisualizationsPy.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.workerVisualizationsPy.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: worker-visualizations-py
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Worker Visualizations Python selector labels
*/}}
{{- define "semantic-explorer.workerVisualizationsPy.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-worker-visualizations-py
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: worker-visualizations-py
{{- end }}

{{/*
Worker Visualizations Python service account name
*/}}
{{- define "semantic-explorer.workerVisualizationsPy.serviceAccountName" -}}
{{- if .Values.workerVisualizationsPy.serviceAccount.create }}
{{- default (printf "%s-worker-visualizations-py" (include "semantic-explorer.fullname" .)) .Values.workerVisualizationsPy.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.workerVisualizationsPy.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Get the full image name for Worker Visualizations Python
*/}}
{{- define "semantic-explorer.workerVisualizationsPy.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.workerVisualizationsPy.image.registry "global" .Values.global) }}
{{- $repository := .Values.workerVisualizationsPy.image.repository }}
{{- $tag := .Values.workerVisualizationsPy.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Get storage class name
*/}}
{{- define "semantic-explorer.storageClass" -}}
{{- if .storageClass }}
{{- .storageClass }}
{{- else if .global.storageClass }}
{{- .global.storageClass }}
{{- end }}
{{- end }}

{{/*
PostgreSQL host
*/}}
{{- define "semantic-explorer.postgresql.host" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.host }}
{{- else }}
{{- printf "%s-postgresql" (include "semantic-explorer.fullname" .) }}
{{- end }}
{{- end }}

{{/*
PostgreSQL port
*/}}
{{- define "semantic-explorer.postgresql.port" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.port }}
{{- else }}
{{- "5432" }}
{{- end }}
{{- end }}

{{/*
PostgreSQL database
*/}}
{{- define "semantic-explorer.postgresql.database" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.database }}
{{- else }}
{{- .Values.postgresql.auth.database }}
{{- end }}
{{- end }}

{{/*
PostgreSQL username
*/}}
{{- define "semantic-explorer.postgresql.username" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.username }}
{{- else }}
{{- .Values.postgresql.auth.username }}
{{- end }}
{{- end }}

{{/*
NATS URL
*/}}
{{- define "semantic-explorer.nats.url" -}}
{{- if .Values.nats.external.enabled }}
{{- .Values.nats.external.url }}
{{- else }}
{{- printf "nats://%s-nats:%d" (include "semantic-explorer.fullname" .) (int .Values.nats.service.client.port | default 4222) }}
{{- end }}
{{- end }}

{{/*
Qdrant URL
*/}}
{{- define "semantic-explorer.qdrant.url" -}}
{{- if .Values.qdrant.external.enabled }}
{{- .Values.qdrant.external.url }}
{{- else }}
{{- printf "http://%s-qdrant:%d" (include "semantic-explorer.fullname" .) (int .Values.qdrant.service.grpc.port | default 6334) }}
{{- end }}
{{- end }}

{{/*
Quickwit URL
*/}}
{{- define "semantic-explorer.quickwit.url" -}}
{{- if .Values.observability.quickwit.external.enabled }}
{{- .Values.observability.quickwit.external.url }}
{{- else }}
{{- printf "http://%s-quickwit:%d" (include "semantic-explorer.fullname" .) (int .Values.observability.quickwit.service.rest.port | default 7280) }}
{{- end }}
{{- end }}

{{/*
Quickwit labels
*/}}
{{- define "semantic-explorer.quickwit.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.quickwit.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: quickwit
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Quickwit selector labels
*/}}
{{- define "semantic-explorer.quickwit.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-quickwit
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: quickwit
{{- end }}

{{/*
Quickwit image
*/}}
{{- define "semantic-explorer.quickwit.image" -}}
{{- $registry := .Values.observability.quickwit.image.registry | default .Values.global.imageRegistry }}
{{- $repository := .Values.observability.quickwit.image.repository }}
{{- $tag := .Values.observability.quickwit.image.tag }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
OIDC Issuer URL
*/}}
{{- define "semantic-explorer.oidc.issuerUrl" -}}
{{- if .Values.dex.external.enabled }}
{{- .Values.dex.external.issuerUrl }}
{{- else }}
{{- .Values.dex.config.issuer }}
{{- end }}
{{- end }}

{{/*
OpenTelemetry Collector endpoint
*/}}
{{- define "semantic-explorer.otel.endpoint" -}}
{{- if .Values.observability.otelCollector.enabled }}
{{- printf "http://%s-otel-collector:%d" (include "semantic-explorer.fullname" .) (int .Values.observability.otelCollector.service.ports.otlpGrpc) }}
{{- else }}
{{- "" }}
{{- end }}
{{- end }}

{{/*
Return the appropriate apiVersion for HPA
*/}}
{{- define "semantic-explorer.hpa.apiVersion" -}}
{{- if .Capabilities.APIVersions.Has "autoscaling/v2" }}
{{- print "autoscaling/v2" }}
{{- else }}
{{- print "autoscaling/v2beta2" }}
{{- end }}
{{- end }}

{{/*
Return the appropriate apiVersion for PodDisruptionBudget
*/}}
{{- define "semantic-explorer.pdb.apiVersion" -}}
{{- if .Capabilities.APIVersions.Has "policy/v1" }}
{{- print "policy/v1" }}
{{- else }}
{{- print "policy/v1beta1" }}
{{- end }}
{{- end }}

{{/*
Return the appropriate apiVersion for Ingress
*/}}
{{- define "semantic-explorer.ingress.apiVersion" -}}
{{- if .Capabilities.APIVersions.Has "networking.k8s.io/v1" }}
{{- print "networking.k8s.io/v1" }}
{{- else if .Capabilities.APIVersions.Has "networking.k8s.io/v1beta1" }}
{{- print "networking.k8s.io/v1beta1" }}
{{- else }}
{{- print "extensions/v1beta1" }}
{{- end }}
{{- end }}

{{/*
Common annotations
*/}}
{{- define "semantic-explorer.annotations" -}}
{{- with .Values.commonAnnotations }}
{{ toYaml . }}
{{- end }}
{{- end }}
{{/*
Grafana labels
*/}}
{{- define "semantic-explorer.grafana.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.grafana.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: grafana
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Grafana selector labels
*/}}
{{- define "semantic-explorer.grafana.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-grafana
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: grafana
{{- end }}

{{/*
Grafana image
*/}}
{{- define "semantic-explorer.grafana.image" -}}
{{- $registry := .Values.observability.grafana.image.registry | default .Values.global.imageRegistry }}
{{- $repository := .Values.observability.grafana.image.repository }}
{{- $tag := .Values.observability.grafana.image.tag }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Storage S3 endpoint
*/}}
{{- define "semantic-explorer.storage.s3.endpoint" -}}
{{- if .Values.minio.enabled }}
{{- printf "http://%s-minio:%d" (include "semantic-explorer.fullname" .) (int .Values.minio.service.port) }}
{{- else }}
{{- .Values.storage.s3.endpoint }}
{{- end }}
{{- end }}

{{/*
Storage S3 region
*/}}
{{- define "semantic-explorer.storage.s3.region" -}}
{{- .Values.storage.s3.region | default "us-east-1" }}
{{- end }}