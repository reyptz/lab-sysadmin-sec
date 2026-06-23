# Platform Engineering

Portail développeur et infrastructure self-service pour le lab.

## Composants

- **Backstage** : portail développeur avec catalogue de services, templates, documentation.
- **Crossplane** : provisioning d'infrastructure via Kubernetes CRDs.

## Organisation

- `backstage/` — Templates et configuration Backstage.
- `crossplane/` — Compositions et claims Crossplane.

## Cas pratique

Un développeur crée un nouveau microservice via le template Backstage :
1. Repo Git initialisé avec CI/CD.
2. Chart Helm et manifests K8s générés.
3. Dashboards Grafana et alertes Prometheus créées.
4. Base PostgreSQL provisionnée via Crossplane.

Résultat : déploiement en 10 minutes sans ticket manuel.
