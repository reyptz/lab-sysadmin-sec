# DevSecOps Policies

Garde-fous automatisés pour Kubernetes et le cycle de livraison.

## Outils

- **OPA Gatekeeper** : policies d'admission Kubernetes (Rego).
- **Kyverno** : policies Kubernetes YAML-friendly.
- **Trivy** : scan d'images et de code.
- **Checkov** : scan IaC.

## Fichiers

- `gatekeeper/` — Policies OPA Gatekeeper.
- `kyverno/` — Policies Kyverno.
- `trivy/` — Config Trivy pour CI/CD.
- `checkov/` — Config Checkov.

## Cas pratique

- Bloquer un pod `privileged=true` en production.
- Bloquer une image non approuvée.
- Empêcher le déploiement si CVE critique.
