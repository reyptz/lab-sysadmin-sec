# Gestion des Secrets

Décloupage complet des secrets (credentials BDD, tokens) hors du code source.

## Options

Un seul mécanisme : **External Secrets Operator (ESO)**, avec le backend adapté à l'environnement :

1. **AWS Secrets Manager** (cloud) : `secret-store.yaml` ;
2. **HashiCorp Vault** (on-premise) : `secret-store-vault.yaml`.

Les `ExternalSecret` sont identiques ; seul le `SecretStore` change (l'injection par Vault Agent, redondante, a été retirée).

## Recommandation

- Rotation : `refreshInterval` court en production ; rotation côté coffre (AWS SM rotation, moteurs dynamiques Vault).
- socle ne stocke aucun secret : il ne manipule que des références (`env:`, `vault:`, `aws-sm:`, `azure-kv:`, `gcp-sm:`).

## Exemples

- `secret-store.yaml` : configure ESO pour lire AWS Secrets Manager.
- `external-secret.yaml` : crée un Kubernetes Secret synchronisé depuis le vault.
- `vault-policy.hcl` : politique Vault minimale pour l'application ERP.
- `secret-store-vault.yaml` : SecretStore ESO adossé à Vault (on-premise).
