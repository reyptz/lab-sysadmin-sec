# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — AKS, App Service, Container Instances
# ---------------------------------------------------------------------------

# AKS cluster
resource "azurerm_kubernetes_cluster" "main" {
  name                = "${var.project_name}-aks-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  dns_prefix          = "${var.project_name}aks${var.environment}"
  kubernetes_version  = "1.29"

  default_node_pool {
    name            = "default"
    node_count      = 2
    vm_size         = "Standard_B2s"
    vnet_subnet_id  = azurerm_subnet.private[0].id
    os_disk_size_gb = 30
  }

  identity {
    type = "SystemAssigned"
  }

  network_profile {
    network_plugin    = "azure"
    network_policy    = "calico"
    load_balancer_sku = "standard"
  }

  tags = local.tags
}

# App Service Plan
resource "azurerm_service_plan" "main" {
  name                = "${var.project_name}-asp-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  os_type             = "Linux"
  sku_name            = "B1"

  tags = local.tags
}

# Linux Web App
resource "azurerm_linux_web_app" "main" {
  name                = "${var.project_name}-webapp-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  service_plan_id     = azurerm_service_plan.main.id

  site_config {
    always_on = true
    application_stack {
      docker_image     = "nginx"
      docker_image_tag = "alpine"
    }
  }

  https_only = true

  tags = local.tags
}

# Container Instance
resource "azurerm_container_group" "sidecar" {
  name                = "${var.project_name}-aci-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  ip_address_type     = "Private"
  subnet_ids          = [azurerm_subnet.private[1].id]
  os_type             = "Linux"

  container {
    name   = "redis"
    image  = "redis:7-alpine"
    cpu    = "0.5"
    memory = "1.0"

    ports {
      port     = 6379
      protocol = "TCP"
    }
  }

  tags = local.tags
}
