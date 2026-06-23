# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — Key Vault, Defender for Cloud, Sentinel
# ---------------------------------------------------------------------------

# Azure Key Vault
resource "azurerm_key_vault" "main" {
  name                        = "${var.project_name}-kv-${var.environment}"
  location                    = azurerm_resource_group.rg.location
  resource_group_name         = azurerm_resource_group.rg.name
  tenant_id                   = data.azuread_client_config.current.tenant_id
  sku_name                    = "standard"
  soft_delete_retention_days  = 7
  purge_protection_enabled    = true

  enable_rbac_authorization = true

  network_acls {
    default_action = "Deny"
    bypass         = "AzureServices"
    ip_rules       = []
    virtual_network_subnet_ids = [azurerm_subnet.private[0].id]
  }

  tags = local.tags
}

# RBAC : Secret Officer pour le groupe admins
resource "azurerm_role_assignment" "kv_secret_officer" {
  scope                = azurerm_key_vault.main.id
  role_definition_name = "Key Vault Secrets Officer"
  principal_id         = azuread_group.admins.object_id
}

# RBAC : Secret Reader pour le groupe readers
resource "azurerm_role_assignment" "kv_secret_reader" {
  scope                = azurerm_key_vault.main.id
  role_definition_name = "Key Vault Secrets User"
  principal_id         = azuread_group.readers.object_id
}

# Example secret
resource "azurerm_key_vault_secret" "example" {
  name         = "example-secret"
  value        = "ChangeMeInProduction"
  key_vault_id = azurerm_key_vault.main.id

  depends_on = [azurerm_role_assignment.kv_secret_officer]
}

# Microsoft Defender for Cloud (Azure Security Center)
resource "azurerm_security_center_subscription_pricing" "main" {
  count = var.enable_defender ? 1 : 0
  tier  = "Standard"
  resource_type = "VirtualMachines"
}

resource "azurerm_security_center_contact" "main" {
  count = var.enable_defender ? 1 : 0
  email = length(var.alert_emails) > 0 ? var.alert_emails[0] : "security@example.com"
  alert_notifications = true
  alerts_to_admins    = true
}

# Microsoft Sentinel (Log Analytics + SecurityInsights)
resource "azurerm_log_analytics_solution" "sentinel" {
  count                 = var.enable_sentinel ? 1 : 0
  solution_name         = "SecurityInsights"
  location              = azurerm_resource_group.rg.location
  resource_group_name   = azurerm_resource_group.rg.name
  workspace_resource_id = azurerm_log_analytics_workspace.monitor.id
  workspace_name        = azurerm_log_analytics_workspace.monitor.name

  plan {
    publisher = "Microsoft"
    product   = "OMSGallery/SecurityInsights"
  }

  tags = local.tags
}

# Azure Monitor Alert Rule for high CPU
resource "azurerm_monitor_metric_alert" "high_cpu" {
  name                = "${var.project_name}-high-cpu-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  scopes              = azurerm_linux_virtual_machine.app[*].id
  description         = "Alerte SRE : CPU > 80% sur les VMs applicatives"

  criteria {
    metric_namespace = "Microsoft.Compute/virtualMachines"
    metric_name      = "Percentage CPU"
    aggregation      = "Average"
    operator         = "GreaterThan"
    threshold        = 80
  }

  action {
    action_group_id = azurerm_monitor_action_group.main.id
  }

  tags = local.tags
}

resource "azurerm_monitor_action_group" "main" {
  name                = "${var.project_name}-actiongroup-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  short_name          = "labsre"

  email_receiver {
    name          = "securityteam"
    email_address = length(var.alert_emails) > 0 ? var.alert_emails[0] : "security@example.com"
  }
}
