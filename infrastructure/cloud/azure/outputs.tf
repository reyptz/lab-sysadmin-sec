output "resource_group_name" {
  description = "Nom du Resource Group"
  value       = azurerm_resource_group.rg.name
}

output "vnet_id" {
  description = "ID du VNet"
  value       = azurerm_virtual_network.vnet.id
}

output "bastion_public_ip" {
  description = "IP publique du bastion"
  value       = azurerm_public_ip.bastion.ip_address
}

output "app_private_ips" {
  description = "IPs privees des serveurs applicatifs"
  value       = azurerm_linux_virtual_machine.app[*].private_ip_address
}

output "log_analytics_workspace_id" {
  description = "ID de l'espace Log Analytics"
  value       = azurerm_log_analytics_workspace.monitor.id
}

output "backup_storage_account" {
  description = "Compte de stockage de backups"
  value       = azurerm_storage_account.backups.name
}

output "aks_cluster_name" {
  description = "Nom du cluster AKS"
  value       = azurerm_kubernetes_cluster.main.name
}

output "web_app_name" {
  description = "Nom de la Web App"
  value       = azurerm_linux_web_app.main.name
}

output "web_app_default_hostname" {
  description = "Hostname par defaut de la Web App"
  value       = azurerm_linux_web_app.main.default_hostname
}

output "sql_server_name" {
  description = "Nom du serveur SQL Azure"
  value       = azurerm_mssql_server.main.name
}

output "sql_database_name" {
  description = "Nom de la base SQL"
  value       = azurerm_mssql_database.main.name
}

output "cosmos_db_account_name" {
  description = "Nom du compte Cosmos DB"
  value       = azurerm_cosmosdb_account.main.name
}

output "data_lake_storage_account" {
  description = "Compte de stockage Data Lake Gen2"
  value       = azurerm_storage_account.datalake.name
}

output "load_balancer_public_ip" {
  description = "IP publique du Load Balancer"
  value       = azurerm_public_ip.lb.ip_address
}

output "front_door_endpoint" {
  description = "Endpoint Front Door (si active)"
  value       = length(azurerm_cdn_frontdoor_endpoint.main) > 0 ? azurerm_cdn_frontdoor_endpoint.main[0].host_name : ""
}

output "key_vault_name" {
  description = "Nom du Key Vault"
  value       = azurerm_key_vault.main.name
}

output "recovery_services_vault_name" {
  description = "Nom du Recovery Services Vault"
  value       = azurerm_recovery_services_vault.main.name
}

output "entra_admin_group_id" {
  description = "ID du groupe Entra ID admins"
  value       = azuread_group.admins.object_id
}

output "entra_reader_group_id" {
  description = "ID du groupe Entra ID readers"
  value       = azuread_group.readers.object_id
}

output "management_group_id" {
  description = "ID du Management Group du lab"
  value       = azurerm_management_group.lab.id
}
