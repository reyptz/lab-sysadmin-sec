# Crossplane — Infrastructure as Code via Kubernetes

Compositions et claims Crossplane pour provisionner des ressources cloud.

## Fichiers

- `compositions/postgresql.yaml` — Composition PostgreSQL managée.
- `claims/postgresql.yaml` — Exemple de claim pour un développeur.
- `providers/provider-config.yaml` — Configuration du provider (AWS/GCP/Azure).

## Utilisation

```bash
# Installer Crossplane
kubectl apply -k https://github.com/crossplane/crossplane/cluster?ref=master

# Appliquer la composition
kubectl apply -f platform/platform-engineering/crossplane/compositions/postgresql.yaml

# Provisionner une base
kubectl apply -f platform/platform-engineering/crossplane/claims/postgresql.yaml
```
