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
NATS URL (uses subchart naming convention)
*/}}
{{- define "semantic-explorer.nats.url" -}}
{{- if .Values.nats.external.enabled }}
{{- .Values.nats.external.url }}
{{- else }}
{{- printf "nats://%s-nats:%d" .Release.Name 4222 }}
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
Storage S3 force path style
Returns "true" if forcePathStyle is enabled or if using MinIO (which requires it)
*/}}
{{- define "semantic-explorer.storage.s3.forcePathStyle" -}}
{{- if .Values.minio.enabled }}
{{- "true" }}
{{- else }}
{{- .Values.storage.s3.forcePathStyle | default false | toString }}
{{- end }}
{{- end }}

{{/*
Storage S3 region
*/}}
{{- define "semantic-explorer.storage.s3.region" -}}
{{- .Values.storage.s3.region | default "us-east-1" }}
{{- end }}

{{/*
Init container image (secure, non-root busybox)
*/}}
{{- define "semantic-explorer.initContainer.image" -}}
{{- .Values.global.initContainer.image | default "busybox:1.36" }}
{{- end }}

{{/*
Init container security context (non-root)
*/}}
{{- define "semantic-explorer.initContainer.securityContext" -}}
runAsNonRoot: true
runAsUser: 65534
runAsGroup: 65534
allowPrivilegeEscalation: false
readOnlyRootFilesystem: true
capabilities:
  drop:
    - ALL
seccompProfile:
  type: RuntimeDefault
{{- end }}

{{/*
PostgreSQL host for init container
*/}}
{{- define "semantic-explorer.postgresql.hostOnly" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.host }}
{{- else }}
{{- printf "%s-postgresql" (include "semantic-explorer.fullname" .) }}
{{- end }}
{{- end }}

{{/*
PostgreSQL port for init container
*/}}
{{- define "semantic-explorer.postgresql.portOnly" -}}
{{- if .Values.postgresql.external.enabled }}
{{- .Values.postgresql.external.port | default 5432 }}
{{- else }}
{{- 5432 }}
{{- end }}
{{- end }}

{{/*
NATS host for init container (subchart naming convention)
*/}}
{{- define "semantic-explorer.nats.host" -}}
{{- if .Values.nats.external.enabled }}
{{- .Values.nats.external.url | trimPrefix "nats://" | regexFind "^[^:]+" }}
{{- else }}
{{- printf "%s-nats" .Release.Name }}
{{- end }}
{{- end }}

{{/*
NATS port for init container
*/}}
{{- define "semantic-explorer.nats.port" -}}
{{- 4222 }}
{{- end }}

{{/*
Qdrant host for init container
*/}}
{{- define "semantic-explorer.qdrant.host" -}}
{{- if .Values.qdrant.external.enabled }}
{{- .Values.qdrant.external.url | trimPrefix "http://" | trimPrefix "https://" | regexFind "^[^:]+" }}
{{- else }}
{{- printf "%s-qdrant" .Release.Name }}
{{- end }}
{{- end }}

{{/*
Qdrant port for init container
*/}}
{{- define "semantic-explorer.qdrant.port" -}}
{{- 6334 }}
{{- end }}

{{/*
MinIO host for init container
*/}}
{{- define "semantic-explorer.minio.host" -}}
{{- if .Values.minio.enabled }}
{{- printf "%s-minio" .Release.Name }}
{{- else }}
{{- .Values.storage.s3.endpoint | trimPrefix "http://" | trimPrefix "https://" | regexFind "^[^:/]+" }}
{{- end }}
{{- end }}

{{/*
MinIO port for init container
*/}}
{{- define "semantic-explorer.minio.port" -}}
{{- if .Values.minio.enabled }}
{{- .Values.minio.service.port | default 9000 }}
{{- else }}
{{- 9000 }}
{{- end }}
{{- end }}

{{/*
Quickwit headless service name
*/}}
{{- define "semantic-explorer.quickwit.headlessService" -}}
{{- printf "%s-quickwit-headless" (include "semantic-explorer.fullname" .) }}
{{- end }}

{{/*
Quickwit peer seeds for clustering (generates comma-separated list of peer addresses)
*/}}
{{- define "semantic-explorer.quickwit.peerSeeds" -}}
{{- $fullname := include "semantic-explorer.fullname" . -}}
{{- $headless := include "semantic-explorer.quickwit.headlessService" . -}}
{{- $replicas := int (.Values.observability.quickwit.replicaCount | default 2) -}}
{{- $port := int (.Values.observability.quickwit.service.rest.port | default 7280) -}}
{{- $seeds := list -}}
{{- range $i := until $replicas -}}
{{- $seeds = append $seeds (printf "%s-quickwit-%d.%s:%d" $fullname $i $headless $port) -}}
{{- end -}}
{{- join "," $seeds -}}
{{- end }}

{{/*
Quickwit host for init container
*/}}
{{- define "semantic-explorer.quickwit.host" -}}
{{- if .Values.observability.quickwit.external.enabled }}
{{- .Values.observability.quickwit.external.url | trimPrefix "http://" | trimPrefix "https://" | regexFind "^[^:/]+" }}
{{- else }}
{{- printf "%s-quickwit" (include "semantic-explorer.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Quickwit gRPC port for init container
*/}}
{{- define "semantic-explorer.quickwit.grpcPort" -}}
{{- .Values.observability.quickwit.service.grpc.port | default 7281 }}
{{- end }}
{{/*
Inference API labels
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.embeddingInferenceApi.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: embedding-inference-api
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Inference API selector labels
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-embedding-inference-api
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: embedding-inference-api
{{- end }}

{{/*
Inference API service account name
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.serviceAccountName" -}}
{{- if .Values.embeddingInferenceApi.serviceAccount.create }}
{{- default (printf "%s-embedding-inference-api" (include "semantic-explorer.fullname" .)) .Values.embeddingInferenceApi.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.embeddingInferenceApi.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Inference API image
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.embeddingInferenceApi.image.registry "global" .Values.global) }}
{{- $repository := .Values.embeddingInferenceApi.image.repository }}
{{- $tag := .Values.embeddingInferenceApi.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}
{{/*
==================================================================================
LLM Inference API Helpers
==================================================================================
*/}}

{{/*
LLM Inference API labels
*/}}
{{- define "semantic-explorer.llmInferenceApi.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.llmInferenceApi.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: llm-inference-api
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
LLM Inference API selector labels
*/}}
{{- define "semantic-explorer.llmInferenceApi.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-llm-inference-api
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: llm-inference-api
{{- end }}

{{/*
LLM Inference API service account name
*/}}
{{- define "semantic-explorer.llmInferenceApi.serviceAccountName" -}}
{{- if .Values.llmInferenceApi.serviceAccount.create }}
{{- default (printf "%s-llm-inference-api" (include "semantic-explorer.fullname" .)) .Values.llmInferenceApi.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.llmInferenceApi.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
LLM Inference API image
*/}}
{{- define "semantic-explorer.llmInferenceApi.image" -}}
{{- $registry := include "semantic-explorer.imageRegistry" (dict "registry" .Values.llmInferenceApi.image.registry "global" .Values.global) }}
{{- $repository := .Values.llmInferenceApi.image.repository }}
{{- $tag := .Values.llmInferenceApi.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
==================================================================================
Dex (OIDC Provider) Helpers
==================================================================================
*/}}

{{/*
Dex labels
*/}}
{{- define "semantic-explorer.dex.labels" -}}
helm.sh/chart: {{ include "semantic-explorer.chart" . }}
{{ include "semantic-explorer.dex.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: dex
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Dex selector labels
*/}}
{{- define "semantic-explorer.dex.selectorLabels" -}}
app.kubernetes.io/name: {{ include "semantic-explorer.name" . }}-dex
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: dex
{{- end }}

{{/*
Dex service account name
*/}}
{{- define "semantic-explorer.dex.serviceAccountName" -}}
{{- default (printf "%s-dex" (include "semantic-explorer.fullname" .)) .Values.dex.serviceAccount.name | default "default" }}
{{- end }}

{{/*
Dex image
*/}}
{{- define "semantic-explorer.dex.image" -}}
{{- $registry := .Values.dex.image.registry | default .Values.global.imageRegistry }}
{{- $repository := .Values.dex.image.repository }}
{{- $tag := .Values.dex.image.tag }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Dex host (for init container dependencies)
*/}}
{{- define "semantic-explorer.dex.host" -}}
{{- if .Values.dex.external.enabled }}
{{- .Values.dex.external.issuerUrl | trimPrefix "http://" | trimPrefix "https://" | regexFind "^[^:/]+" }}
{{- else }}
{{- printf "%s-dex" (include "semantic-explorer.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Dex port (for init container dependencies)
*/}}
{{- define "semantic-explorer.dex.port" -}}
{{- .Values.dex.service.port | default 5556 }}
{{- end }}

{{/*
==================================================================================
Inference API URL Helpers
==================================================================================
*/}}

{{/*
Embedding Inference API URL
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.url" -}}
{{- if .Values.embeddingInferenceApi.enabled }}
{{- printf "http://%s-embedding-inference-api:%d" (include "semantic-explorer.fullname" .) (int .Values.embeddingInferenceApi.service.port) }}
{{- else }}
{{- "" }}
{{- end }}
{{- end }}

{{/*
Embedding Inference API host (for init container)
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.host" -}}
{{- printf "%s-embedding-inference-api" (include "semantic-explorer.fullname" .) }}
{{- end }}

{{/*
Embedding Inference API port (for init container)
*/}}
{{- define "semantic-explorer.embeddingInferenceApi.port" -}}
{{- .Values.embeddingInferenceApi.service.port | default 8090 }}
{{- end }}

{{/*
LLM Inference API URL
*/}}
{{- define "semantic-explorer.llmInferenceApi.url" -}}
{{- if .Values.llmInferenceApi.enabled }}
{{- printf "http://%s-llm-inference-api:%d" (include "semantic-explorer.fullname" .) (int .Values.llmInferenceApi.service.port) }}
{{- else }}
{{- "" }}
{{- end }}
{{- end }}

{{/*
LLM Inference API host (for init container)
*/}}
{{- define "semantic-explorer.llmInferenceApi.host" -}}
{{- printf "%s-llm-inference-api" (include "semantic-explorer.fullname" .) }}
{{- end }}

{{/*
LLM Inference API port (for init container)
*/}}
{{- define "semantic-explorer.llmInferenceApi.port" -}}
{{- .Values.llmInferenceApi.service.port | default 8091 }}
{{- end }}
