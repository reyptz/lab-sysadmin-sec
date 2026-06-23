# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — Site Recovery, Backup, geo-replication
# ---------------------------------------------------------------------------

# Recovery Services Vault
resource "azurerm_recovery_services_vault" "main" {
  name                = "${var.project_name}-rsv-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  sku                 = "Standard"
  soft_delete_enabled = true

  tags = local.tags
}

# Backup Policy VM
resource "azurerm_backup_policy_vm" "main" {
  name                = "${var.project_name}-backup-policy-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  recovery_vault_name = azurerm_recovery_services_vault.main.name

  backup {
    frequency = "Daily"
    time      = "02:00"
  }

  retention_daily {
    count = 7
  }
}

# Backup des VMs applicatives
resource "azurerm_backup_protected_vm" "app" {
  count               = length(azurerm_linux_virtual_machine.app)
  resource_group_name = azurerm_resource_group.rg.name
  recovery_vault_name = azurerm_recovery_services_vault.main.name
  source_vm_id        = azurerm_linux_virtual_machine.app[count.index].id
  backup_policy_id    = azurerm_backup_policy_vm.main.id
}

# Geo-replication Storage Account backups
resource "azurerm_storage_account" "backups_geo" {
  name                     = lower(substr(replace("${var.project_name}backupsgeo${var.environment}", "-", ""), 0, 24))
  resource_group_name      = azurerm_resource_group.rg.name
  location                 = var.azure_secondary_region
  account_tier             = "Standard"
  account_replication_type = "GRS"
  min_tls_version          = "TLS1_2"

  tags = local.tags
}

# Site Recovery pour la VM bastion (replication vers region secondaire)
resource "azurerm_site_recovery_fabric" "primary" {
  count                                      = var.enable_site_recovery ? 1 : 0
  name                                       = "${var.project_name}-primary-fabric"
  resource_group_name                        = azurerm_resource_group.rg.name
  recovery_vault_name                        = azurerm_recovery_services_vault.main.name
  location                                   = azurerm_resource_group.rg.location
}

resource "azurerm_site_recovery_fabric" "secondary" {
  count                                      = var.enable_site_recovery ? 1 : 0
  name                                       = "${var.project_name}-secondary-fabric"
  resource_group_name                        = azurerm_resource_group.rg.name
  recovery_vault_name                        = azurerm_recovery_services_vault.main.name
  location                                   = var.azure_secondary_region
}

resource "null_resource" "asr_note" {
  triggers = {
    note = <<EOF
Azure Site Recovery (ASR) : replication de VMs vers une region secondaire.
Ce lab cree les fabrics ; pour activer la replication complete il faut aussi :
- azurerm_site_recovery_protection_container (primary + secondary)
- azurerm_site_recovery_replication_policy
- azurerm_site_recovery_protection_container_mapping
- azurerm_site_recovery_replicated_vm
Ces ressources sont volontairement laissees en documentation pour eviter les couts ASR eleves.
EOF
  }
}
