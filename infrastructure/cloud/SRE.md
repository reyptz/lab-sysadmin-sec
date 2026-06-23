# SRE & Observabilité du Lab Cloud

Ce document applique les pratiques **Site Reliability Engineering (SRE)** du parcours **Google Cloud Professional Cloud DevOps Engineer** et du **AWS Solutions Architect Professional**.

## SLIs / SLOs / SLAs

### Infrastructure cloud (IaaS)

| Service | SLI | SLO | SLA cible |
|---------|-----|-----|-----------|
| Bastion SSH | Disponibilité du port 22 | 99,9 % sur 30 jours | 99,5 % |
| Serveurs applicatifs (HTTP/HTTPS) | Taux de reussite des requetes | 99,95 % sur 30 jours | 99,9 % |
| Logs centralisés | Latence d'ingestion | < 5 min | < 10 min |
| Backups | RPO (Recovery Point Objective) | < 24 h | < 48 h |
| Temps de reponse | Latence p95 | < 500 ms | < 1 s |

### Application (SaaS / Kubernetes)

| Service | SLI | SLO (30 jours) | Error Budget | SLA externe |
|---------|-----|---------------|--------------|-------------|
| API Application (`/api/**`) | Disponibilité (taux de requêtes 2xx/3xx) | 99,95 % | 21,6 min/mois | 99,9 % |
| Portail client web | Disponibilité | 99,99 % | 4,3 min/mois | 99,95 % |
| Authentification (Keycloak) | Disponibilité | 99,99 % | 4,3 min/mois | 99,9 % |
| API IA (modules IA) | Disponibilité | 99,9 % | 43,8 min/mois | 99,5 % |
| Latence API Application | p95 < 300 ms | 99 % des requêtes | — | p95 < 500 ms |
| Traitement batch paiement | Taux de succès | 99,999 % | 26,3 s/mois | 99,99 % |
| Ingestion logs/métriques | Latence < 30 s | 99,9 % | — | < 2 min |

## Error Budgets

L'**error budget** est le temps d'indisponibilité autorisé avant de déclencher des actions correctives (gel des releases, rollback, etc.).

| SLO | Error Budget mensuel | Action si dépassé |
|-----|----------------------|-------------------|
| 99,9 % | 43,8 min | Alerte équipe SRE, investigation immédiate. |
| 99,95 % | 21,6 min | Gel des déploiements non critiques. |
| 99,99 % | 4,3 min | Incident majeur, rollback automatique, post-mortem. |
| 99,999 % | 26,3 s | War room, escalation management, audit régulateur. |

### Règles de gouvernance des error budgets

1. **Si 50 % de l'error budget est consommé en 1 semaine** : gel des nouvelles features, focus sur la stabilité.
2. **Si 100 % de l'error budget est consommé** : interdiction de déployer en production (sauf hotfix de sécurité).
3. **Budget renouvelé** : le 1er de chaque mois.

## Alertes principales

| Alerte | Cloud | Seuil | Action |
|--------|-------|-------|----------|
| CPU eleve | AWS CloudWatch | > 80 % sur 5 min | Scale ou investigation |
| CPU eleve | GCP Cloud Monitoring | > 80 % sur 5 min | Scale ou investigation |
| Echec de santé | Azure Monitor | VM non disponible | PagerDuty / ticket |
| Taux d'erreur HTTP | Application Load Balancer | > 1 % | Rollback ou investigation |

## Runbooks

### Bastion inaccessible
1. Verifier le Security Group / NSG / Firewall Rule (port 22, source CIDR).
2. Verifier l'etat de l'instance dans la console.
3. Utiliser la console serie (AWS EC2 serial / Azure serial console / GCP serial port).
4. Redemarrer si necessaire ; analyser les logs d'audit.

### Serveur applicatif en panne
1. Verifier la connectivite depuis le bastion.
2. Consulter les logs applicatifs dans CloudWatch / Azure Monitor / Cloud Logging.
3. Verifier les metriques CPU / memoire / disque.
4. Redemarrer le service ou redeployer via Ansible / cloud-init.

### Suspicion de compromission
1. Isoler la VM (changer le Security Group / NSG / Firewall pour bloquer le trafic entrant/sortant).
2. Capturer un snapshot du disque.
3. Analyser les logs via Wazuh / SIEM cloud.
4. Appliquer les playbooks Ansible de hardening.

## Couts & optimisation

- Arreter les environnements de dev en dehors des heures de travail (scheduled stop/start).
- Utiliser des instances spot/preemptible pour les workloads tolerants.
- Activer les budgets et alertes de facturation sur chaque cloud.
- Review mensuelle des ressources inutilisees ( unattached IPs, vieux snapshots, buckets vides ).

## SLO Monitoring & Dashboards

### Requêtes PromQL de base

```promql
# Disponibilité API Application (30 jours)
sum(rate(app_http_requests_total{status!~"5.."}[30d]))
/
sum(rate(app_http_requests_total[30d]))

# Latence p95 API Application
histogram_quantile(0.95, sum(rate(app_http_request_duration_seconds_bucket[5m])) by (le))

# Error budget consommé (99,95 % SLO)
1 - (
  sum(rate(app_http_requests_total{status!~"5.."}[30d]))
  /
  sum(rate(app_http_requests_total[30d]))
) - 0.0005
```

### Dashboards Grafana recommandés

- **SRE Golden Signals** : traffic, latency, errors, saturation.
- **Error Budget Burn** : consommation du budget mensuel par service.
- **Canary Analysis** : comparaison stable vs canary (Argo Rollouts).
- **Multi-Tenant Health** : santé par namespace/tenant.

## Chaos Engineering & Postmortem Culture

Les pratiques de resilience et d'amelioration continue sont documentees dans [`docs/gcp_chaos_engineering.md`](../../docs/gcp_chaos_engineering.md) :

- **Chaos Engineering** : experimentations planifiees sur GKE (Litmus/Chaos Mesh), GCE (Chaos Monkey), Cloud Run (latence reseau).
- **Blameless Postmortems** : template standard, action items, review periodique.
- **Game Days** : calendrier mensuel d'experimentation et de validation des runbooks.

## Checklist de production

- [ ] Backend Terraform partage et verrouille (S3 + DynamoDB / GCS / Azure Blob).
- [ ] VPC Flow Logs / equivalent actives.
- [ ] Chiffrement au repos et en transit partout.
- [ ] MFA sur les comptes cloud.
- [ ] Rotation des secrets via Vault / Secret Manager / Key Vault.
- [ ] Backups testes regulierement (restore drill).
- [ ] DR plan documente avec RTO/RPO valides.
- [ ] SLI/SLO définis et monitorés dans Grafana.
- [ ] Error Budget policy approuvée par l'équipe produit.
- [ ] Runbooks et playbooks d'incident à jour.
