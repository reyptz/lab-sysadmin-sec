# Gestion des Secrets

Décloupage complet des secrets (credentials BDD, tokens) hors du code source.

## Options

1. **External Secrets Operator (ESO)** : synchronise les secrets depuis AWS Secrets Manager vers des Kubernetes Secrets.
2. **HashiCorp Vault + Vault Agent Injector** : injecte les secrets directement dans les pods sans jamais les stocker en tant que Secret Kubernetes.

## Recommandation

- **En production** : utiliser Vault avec injection dynamique pour les credentials BDD et les tokens API.
- **En développement** : ESO + AWS Secrets Manager pour simplifier le workflow.

## Exemples

- `secret-store.yaml` : configure ESO pour lire AWS Secrets Manager.
- `external-secret.yaml` : crée un Kubernetes Secret synchronisé depuis le vault.
- `vault-policy.hcl` : politique Vault minimale pour l'application ERP.
- `vault-agent-inject.yaml` : annotation d'injection Vault Agent.
