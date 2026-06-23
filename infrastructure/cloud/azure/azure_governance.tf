# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — Management Groups, Azure Policy, Blueprints
# ---------------------------------------------------------------------------

# Management Group dedie au lab
resource "azurerm_management_group" "lab" {
  display_name               = "${var.project_name}-${var.environment}"
  name                       = "${var.project_name}-${var.environment}"
  parent_management_group_id = data.azurerm_management_group.root.id
}

# Subscription associee au management group du lab
resource "azurerm_management_group_subscription_association" "lab" {
  management_group_id = azurerm_management_group.lab.id
  subscription_id   = data.azurerm_subscription.current.id
}

# Initiative / Policy Set : gouvernance et securite de base
resource "azurerm_policy_set_definition" "security_baseline" {
  name         = "${var.project_name}-security-baseline-${var.environment}"
  policy_type  = "Custom"
  display_name = "Security Baseline for Lab"
  description  = "Initiative regroupant les regles de gouvernance et securite de base"

  policy_definition_reference {
    policy_definition_id = data.azurerm_policy_definition.require_tag.id
  }

  policy_definition_reference {
    policy_definition_id = data.azurerm_policy_definition.inherit_tag.id
  }

  policy_definition_reference {
    policy_definition_id = data.azurerm_policy_definition.allowed_locations.id
  }
}

# Assignment de l'initiative sur le management group
resource "azurerm_policy_assignment" "security_baseline" {
  name                 = "${var.project_name}-security-baseline-${var.environment}"
  display_name         = "Security Baseline Assignment"
  policy_definition_id = azurerm_policy_set_definition.security_baseline.id
  scope                = azurerm_management_group.lab.id
  location             = var.azure_region

  identity {
    type = "SystemAssigned"
  }
}

# Azure Blueprints
# Note : Azure Blueprints est en mode maintenance ; Microsoft recommande Deployment Stacks ou Template Specs.
# On fournit un exemple Template Spec a la place.
resource "azurerm_resource_group_template_deployment" "blueprint" {
  name                = "${var.project_name}-blueprint-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  deployment_mode     = "Incremental"

  template_content = jsonencode({
    "$schema"      = "https://schema.management.azure.com/schemas/2019-04-01/deploymentTemplate.json#"
    contentVersion = "1.0.0.0"
    resources = [
      {
        type       = "Microsoft.Network/networkSecurityGroups"
        apiVersion = "2023-04-01"
        name       = "${var.project_name}-blueprint-nsg"
        location   = var.azure_region
        properties = {
          securityRules = [
            {
              name = "DenyInternetSSH"
              properties = {
                priority                   = 100
                direction                  = "Inbound"
                access                     = "Deny"
                protocol                   = "Tcp"
                sourcePortRange            = "*"
                destinationPortRange       = "22"
                sourceAddressPrefix        = "Internet"
                destinationAddressPrefix   = "*"
              }
            }
          ]
        }
      }
    ]
  })

  tags = local.tags
}

# Data source : root management group
 data "azurerm_management_group" "root" {
  display_name = "Tenant Root Group"
}

 data "azurerm_policy_definition" "require_tag" {
  display_name = "Require a tag on resources"
}

 data "azurerm_policy_definition" "inherit_tag" {
  display_name = "Inherit a tag from the resource group"
}

 data "azurerm_policy_definition" "allowed_locations" {
  display_name = "Allowed locations"
}
