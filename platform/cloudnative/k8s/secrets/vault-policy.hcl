# Politique Vault minimale pour l'application ERP
path "secret/data/erp/prod/db" {
  capabilities = ["read"]
}

path "secret/data/erp/prod/ai-api-key" {
  capabilities = ["read"]
}

# Interdit explicitement toute ecriture
path "secret/*" {
  capabilities = ["deny"]
}
