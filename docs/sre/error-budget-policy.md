# Error Budget Policy — API Facturation

## SLO

- **Disponibilité** : 99,9 % sur 30 jours pour `GET /invoices`.
- **Error budget** : 0,1 % = ~43 minutes d'indisponibilité acceptable.

## Règles de gouvernance

| Budget consommé | Fenêtre | Action |
|---|---|---|
| 25 % | 1 jour | Notification Slack, pas d'action immédiate. |
| 50 % | 1 semaine | Gel des releases non critiques. Focus stabilité. |
| 75 % | 2 semaines | Incident review obligatoire. Escalade SRE. |
| 100 % | Avant fin de mois | Interdiction de déployer en production (sauf hotfix sécurité). |

## Burn rate alerts

- **Fast burn (2h)** : si le budget est consommé à 14,4x sur 2h, alerte critique.
- **Slow burn (6h)** : si le budget est consommé à 2x sur 6h, alerte warning.

## Conséquences

- Gel des releases : les features planifiées sont reportées au mois suivant.
- Priorité au bugfixing et aux améliorations de fiabilité.
- Postmortem obligatoire si 100 % du budget est consommé.

## Renouvellement

Le budget est renouvelé le 1er de chaque mois à minuit UTC.
