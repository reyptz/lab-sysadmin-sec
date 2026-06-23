# Multi-Tenant Kubernetes Platform

Isolation stricte des environnements et des données clients pour un ERP/CRM fintech.

## Principes

- **Namespaces dédiés** : un namespace par environnement (`erp-dev`, `erp-staging`, `erp-prod`).
- **RBAC least privilege** : service account minimal pour l'application ERP.
- **Network Policies** : refus par défaut, autorisation explicite entre microservices.
- **Pod Security Standards** : pods non-root, filesystem read-only, capabilities drop all.

## Appliquer

```bash
kubectl apply -f platform/cloudnative/k8s/tenancy/namespaces.yaml
kubectl apply -f platform/cloudnative/k8s/tenancy/rbac/
kubectl apply -f platform/cloudnative/k8s/tenancy/network-policies/
```

## Isolation multi-tenant

Pour isoler des clients dans un même cluster, créer un namespace par tenant :

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: erp-tenant-client-a
  labels:
    tenant: client-a
    env: prod
```

Associer des NetworkPolicies, RBAC et ResourceQuotas pour limiter les ressources et les flux.
