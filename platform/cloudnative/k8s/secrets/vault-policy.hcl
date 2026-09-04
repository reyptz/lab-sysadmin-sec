# Politique Vault minimale pour l'application ERP
path "secret/data/erp/prod/db" {
  capabilities = ["read"]
}

# Interdit explicitement toute ecriture
path "secret/*" {
  capabilities = ["deny"]
}
