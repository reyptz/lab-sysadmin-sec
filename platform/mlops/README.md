# MLOps / AIOps

Industrialisation des modèles ML et exploitation intelligente.

## Cas pratique

**Modèle** : scoring risque impayé.

- Entraînement quotidien.
- Registry de modèles.
- Déploiement KServe avec canary.
- Monitoring : latence, drift, performance.

## Fichiers

- `kserve/inference-service.yaml` — Service d'inférence KServe.
- `kserve/canary.yaml` — Déploiement canary 10 %.
- `pipelines/training-pipeline.yaml` — Pipeline d'entraînement.
- `monitoring/model-metrics.md` — Métriques de modèle et drift.
- `aiops/anomaly-detection.md` — Détection d'anomalies AIOps.

## Prérequis

- KServe installé sur le cluster.
- MLflow ou DVC pour le registry.
- Prometheus/Grafana pour le monitoring.
