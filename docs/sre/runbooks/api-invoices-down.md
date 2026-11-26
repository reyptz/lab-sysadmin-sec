# Runbook — API /invoices Down

## Alerte

`APIInvoicesErrorRateHigh` : error rate > 1 % sur /invoices.

## Impact

- Clients ne peuvent pas lister/créer de factures.
- Consommation de l'error budget.

## Étapes de diagnostic

1. **Vérifier l'état des pods**
   ```bash
   kubectl get pods -n billing -l app=api-invoices
   kubectl describe pod -n billing -l app=api-invoices
   ```

2. **Consulter les logs**
   ```bash
   kubectl logs -n billing -l app=api-invoices --tail=500 | grep ERROR
   ```

3. **Vérifier la base de données**
   ```bash
   kubectl get pods -n billing -l app=postgres
   kubectl logs -n billing -l app=postgres --tail=100
   ```

4. **Vérifier les dépendances**
   - Cache Redis / Keycloak
   - Message queue
   - Network policies

## Actions correctives

1. **Rollback** si une release récente est en cause.
   ```bash
   kubectl rollout undo deployment/api-invoices -n billing
   ```

2. **Scale horizontal** si surcharge.
   ```bash
   kubectl scale deployment/api-invoices -n billing --replicas=5
   ```

3. **Redémarrer le service** si nécessaire.
   ```bash
   kubectl rollout restart deployment/api-invoices -n billing
   ```

4. **Escalader** si compromission suspecte (voir runbook sécurité).

## Vérification

- Error rate < 0,1 % sur 5 min.
- p95 latence < 300ms.
- SLO non dégradé.

## Postmortem

- Si error budget > 25 % consommé, postmortem obligatoire.
