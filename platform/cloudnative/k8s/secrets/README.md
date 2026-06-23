# Gestion des Secrets

Décloupage complet des secrets (clés API IA, credentials BDD) hors du code source.

## Options

1. **External Secrets Operator (ESO)** : synchronise les secrets depuis un vault cloud (AWS Secrets Manager, Azure Key Vault, GCP Secret Manager) vers des Kubernetes Secrets.
2. **HashiCorp Vault + Vault Agent Injector** : injecte les secrets directement dans les pods sans jamais les stocker en tant que Secret Kubernetes.

## Recommandation fintech

- **En production** : utiliser Vault avec injection dynamique pour les credentials BDD et les clés API.
- **En développement** : ESO + AWS Secrets Manager/Azure Key Vault pour simplifier le workflow.

## Exemples

- `secret-store.yaml` : configure ESO pour lire AWS Secrets Manager.
- `external-secret.yaml` : crée un Kubernetes Secret synchronisé depuis le vault.
- `vault-policy.hcl` : politique Vault minimale pour l'application ERP.
- `vault-agent-inject.yaml` : annotation d'injection Vault Agent.
