# Kyverno Policies

Policies Kubernetes plus simples et YAML-friendly.

## Installation

```bash
kubectl apply -f https://github.com/kyverno/kyverno/releases/download/v1.11.0/install.yaml
kubectl apply -f platform/devsecops/policies/kyverno/
```

## Policies

- `require-labels.yaml` — Exige les labels `app`, `team`, `owner`, `cost-center`.
- `require-resources.yaml` — Exige les `resources.requests` et `resources.limits`.
- `restrict-image-registries.yaml` — N'autorise que les registres approuvés.
- `disallow-latest-tag.yaml` — Interdit le tag `latest`.
