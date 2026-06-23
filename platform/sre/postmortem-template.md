# Postmortem Template — [INCIDENT-XXX]

## Résumé

- **Service** : API Facturation
- **Date** : YYYY-MM-DD HH:MM UTC
- **Durée** : XX min
- **Impact** : XX % des requêtes /invoices en échec, XX clients impactés
- **Severity** : SEV-1 / SEV-2 / SEV-3

## Chronologie

| Heure UTC | Événement |
|---|---|
| HH:MM | Début de l'incident (premier 5xx) |
| HH:MM | Alerte Prometheus déclenchée |
| HH:MM | On-call ack |
| HH:MM | Action corrective démarrée |
| HH:MM | Service rétabli |

## Root cause

Description concise de la cause racine.

## Impact sur SLO

- SLO availability : 99,9 % sur 30 jours.
- Error budget consommé : XX minutes / 43 minutes.
- Burn rate max : XXx sur 2h.

## Actions correctives immédiates

- [ ] Rollback de la version X.Y.Z.
- [ ] Mise à l'échelle horizontale.
- [ ] Redémarrage du service DB.

## Actions de fond (blameless)

- [ ] Ajouter un test de charge avant release.
- [ ] Améliorer le circuit breaker.
- [ ] Documenter le runbook.

## Leçons apprises

- Ce qui a fonctionné.
- Ce qui n'a pas fonctionné.
- Ce qu'on change pour la prochaine fois.

## Suivi

- Date de review : YYYY-MM-DD
- Owner : @pseudo
- Tickets Jira : [LINK-1], [LINK-2]
