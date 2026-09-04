# DevSecOps Policies

Garde-fous automatisés pour Kubernetes et le cycle de livraison.

## Outils

- **OPA Gatekeeper** : policies d'admission Kubernetes (Rego).
- **Kyverno** : policies Kubernetes YAML-friendly.
- **Trivy** : scan d'images et de code.
- **Checkov** : scan IaC.

## Fichiers

- `kubernetes/gatekeeper/` — Policies OPA Gatekeeper.
- `kubernetes/kyverno/` — Policies Kyverno.
- `scan/trivy/` — Config Trivy pour CI/CD.
- `iac/checkov/` — Config Checkov.

## Cas pratique

- Bloquer un pod `privileged=true` en production.
- Bloquer une image non approuvée.
- Empêcher le déploiement si CVE critique.
