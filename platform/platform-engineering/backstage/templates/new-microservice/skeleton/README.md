# ${{values.name}}

${{values.description}}

## Démarrage

```bash
npm ci
npm test
npm run dev
```

## Déploiement

```bash
# CI/CD automatique via GitHub Actions
# ArgoCD synchronise le namespace
kubectl get pods -n ${{values.name}}
```

## Dépendances

- PostgreSQL : `postgres-${{values.name}}`
- API Gateway

## SLO

- Disponibilité : 99,9 % sur 30 jours.
- Latence p95 : < 300ms.
