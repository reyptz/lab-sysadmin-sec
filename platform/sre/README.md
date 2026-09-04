# SRE — API Facturation

Exemple concret de SLO / SLI / Error Budget / Alerting pour l'API Facturation (`/invoices`).

## Définitions

| Concept | Valeur |
|---|---|
| **Service** | API Facturation |
| **SLI** | Taux de requêtes HTTP 2xx/3xx sur `/invoices` |
| **SLO** | 99,9 % sur 30 jours |
| **Error Budget** | 0,1 % soit ~43 min d'indisponibilité / 30 jours |
| **Alerting** | Burn rate 2h et 6h |
| **Chaos** | Pod kill, CPU stress, latence DB |

## Fichiers

- `rules/slo-definition.yml` — Définition des SLO/SLI.
- `rules/prometheus-rules.yml` — Règles Prometheus (burn rate, latence, erreurs).
- `dashboards/grafana-dashboard.json` — Dashboard SRE pour API Facturation.
- `error-budget-policy.md` — Politique de gestion du budget d'erreur.
- `postmortem-template.md` — Template de postmortem.

## Commandes

```bash
# Vérifier les règles Prometheus
promtool check rules platform/sre/rules/prometheus-rules.yml

# Appliquer le dashboard
kubectl create configmap grafana-dashboard-invoices --from-file=platform/sre/dashboards/grafana-dashboard.json
```
