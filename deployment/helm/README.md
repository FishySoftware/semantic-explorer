# Semantic Explorer Helm Chart

This directory contains the Helm chart for deploying Semantic Explorer to Kubernetes.

## Installation

### From GitHub Packages (Recommended)

The Helm chart is automatically published to GitHub Packages when a new version tag is pushed.

```bash
# Install a specific version
helm install my-release oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer --version 1.1.0

# Or install the latest version
helm install my-release oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer
```

### From Local Source

To install from the local chart directory:

```bash
helm install my-release ./deployment/helm/semantic-explorer
```

## Configuration

### Default Values

View the default configuration values:

```bash
helm show values oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer --version 1.1.0
```

### Custom Values

Create a custom values file and use it during installation:

```bash
helm install my-release oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer --version 1.1.0 -f custom-values.yaml
```

Or override specific values:

```bash
helm install my-release oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer --version 1.1.0 --set image.tag=latest
```

## Upgrading

To upgrade an existing release:

```bash
helm upgrade my-release oci://ghcr.io/fishysoftware/semantic-explorer/semantic-explorer --version 1.1.0
```

## Uninstalling

To remove the release:

```bash
helm uninstall my-release
```

## Listing Available Versions

To list all available chart versions in the GitHub Container Registry:

```bash
# Using the GitHub CLI
gh api /users/fishysoftware/packages/container/semantic-explorer%2Fsemantic-explorer/versions --jq '.[].metadata.container.tags[]'

# Or view in browser
# https://github.com/FishySoftware/semantic-explorer/pkgs/container/semantic-explorer%2Fsemantic-explorer
```

## Publishing New Versions

The Helm chart is automatically published to GitHub Packages when a version tag is pushed:

```bash
git tag v1.1.0
git push origin v1.1.0
```

The GitHub Actions workflow [`.github/workflows/publish-helm-chart.yaml`](../../.github/workflows/publish-helm-chart.yaml) will automatically package and publish the chart.

## Chart Structure

```
semantic-explorer/
├── Chart.yaml          # Chart metadata
├── values.yaml         # Default configuration values
├── charts/             # Dependency charts
├── templates/          # Kubernetes resource templates
└── .helmignore        # Files to exclude from packaging
```

## Performance & Scalability Configuration

The chart includes configuration for high-availability deployments (100K+ users):

### Circuit Breakers

```yaml
api:
  env:
    QDRANT_CIRCUIT_BREAKER_FAILURE_THRESHOLD: "5"
    QDRANT_CIRCUIT_BREAKER_TIMEOUT_SECS: "30"
    S3_CIRCUIT_BREAKER_FAILURE_THRESHOLD: "5"
    INFERENCE_CIRCUIT_BREAKER_FAILURE_THRESHOLD: "5"
```

### NATS Consumer Tuning

```yaml
api:
  env:
    NATS_MAX_ACK_PENDING: "100"
    NATS_COLLECTION_ACK_WAIT_SECS: "600"
    NATS_DATASET_ACK_WAIT_SECS: "600"
    NATS_VISUALIZATION_ACK_WAIT_SECS: "1800"
```

### Retry Policies

```yaml
workerDatasets:
  env:
    RETRY_MAX_ATTEMPTS: "3"
    RETRY_INITIAL_DELAY_MS: "100"
    QDRANT_RETRY_MAX_ATTEMPTS: "3"
```

## Dependencies

The chart includes the following optional dependencies:

- **NATS** - Message broker for async communication
- **Qdrant** - Vector database for embeddings
- **MinIO** - Object storage
- **Grafana** - Monitoring dashboards
- **Prometheus** - Metrics collection

These can be enabled/disabled in the values file.

## Troubleshooting

### Check Release Status

```bash
helm status my-release
```

### View Release History

```bash
helm history my-release
```

### Debug Installation

```bash
helm install my-release oci://ghcr.io/fishysoftware/semantic-explorer --version 1.1.0 --dry-run --debug
```

## Support

For issues or questions, please visit the [project repository](https://github.com/FishySoftware/semantic-explorer).
