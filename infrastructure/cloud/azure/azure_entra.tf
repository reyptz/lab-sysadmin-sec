# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — Entra ID, RBAC, Conditional Access
# ---------------------------------------------------------------------------

# Groupe Entra ID pour les administrateurs du lab
resource "azuread_group" "admins" {
  display_name     = "${var.project_name}-admins-${var.environment}"
  mail_enabled     = false
  security_enabled = true
  description      = "Administrateurs du lab Azure"

  owners = [data.azuread_client_config.current.object_id]
}

# Groupe Entra ID pour les lecteurs SOC
resource "azuread_group" "readers" {
  display_name     = "${var.project_name}-readers-${var.environment}"
  mail_enabled     = false
  security_enabled = true
  description      = "Lecteurs SOC du lab Azure"

  owners = [data.azuread_client_config.current.object_id]
}

# Application / Service Principal pour les deploiements CI/CD
resource "azuread_application" "cicd" {
  display_name = "${var.project_name}-cicd-${var.environment}"
  owners       = [data.azuread_client_config.current.object_id]
}

resource "azuread_service_principal" "cicd" {
  client_id                    = azuread_application.cicd.client_id
  app_role_assignment_required = false
  owners                       = [data.azuread_client_config.current.object_id]
}

# RBAC : Contributor sur le resource group pour le groupe admins
resource "azurerm_role_assignment" "admins_contributor" {
  scope                = azurerm_resource_group.rg.id
  role_definition_name = "Contributor"
  principal_id         = azuread_group.admins.object_id
}

# RBAC : Reader sur le resource group pour le groupe readers
resource "azurerm_role_assignment" "readers_reader" {
  scope                = azurerm_resource_group.rg.id
  role_definition_name = "Reader"
  principal_id         = azuread_group.readers.object_id
}

# RBAC : Contributor sur le subscription pour le SP CICD (optionnel, a restreindre en prod)
resource "azurerm_role_assignment" "cicd_contributor" {
  scope                = data.azurerm_subscription.current.id
  role_definition_name = "Contributor"
  principal_id       = azuread_service_principal.cicd.object_id
}

# Conditional Access : exiger MFA pour les administrateurs
# Note : requiert une licence Entra ID P1/P2.
resource "azuread_conditional_access_policy" "admins_mfa" {
  display_name = "${var.project_name}-admins-mfa-${var.environment}"
  state        = var.enable_conditional_access ? "enabled" : "disabled"

  conditions {
    applications {
      included_applications = ["All"]
    }
    users {
      included_groups = [azuread_group.admins.object_id]
    }
    locations {
      included_locations = ["AllTrusted"]
    }
    platforms {
      included_platforms = ["all"]
    }
  }

  grant_controls {
    operator          = "OR"
    built_in_controls = ["mfa"]
  }

  session_controls {
    sign_in_frequency = 4
    sign_in_frequency_period = "hours"
  }
}

# Data sources
 data "azuread_client_config" "current" {}
 data "azurerm_subscription" "current" {}
