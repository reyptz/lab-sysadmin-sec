# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — Front Door, Application Gateway, Load Balancer
# ---------------------------------------------------------------------------

# Load Balancer public (Standard) pour les VMs applicatives
resource "azurerm_lb" "app" {
  name                = "${var.project_name}-lb-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  sku                 = "Standard"

  frontend_ip_configuration {
    name                 = "PublicIPAddress"
    public_ip_address_id = azurerm_public_ip.lb.id
  }

  tags = local.tags
}

resource "azurerm_public_ip" "lb" {
  name                = "${var.project_name}-lb-pip"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  allocation_method   = "Static"
  sku                 = "Standard"

  tags = local.tags
}

resource "azurerm_lb_backend_address_pool" "app" {
  name            = "${var.project_name}-lb-backend"
  loadbalancer_id = azurerm_lb.app.id
}

resource "azurerm_lb_backend_address_pool_address" "app" {
  count                   = length(azurerm_linux_virtual_machine.app)
  name                    = "app-${count.index + 1}"
  backend_address_pool_id = azurerm_lb_backend_address_pool.app.id
  ip_address              = azurerm_linux_virtual_machine.app[count.index].private_ip_address
  virtual_network_id      = azurerm_virtual_network.vnet.id
}

resource "azurerm_lb_probe" "http" {
  name            = "http-probe"
  loadbalancer_id = azurerm_lb.app.id
  port            = 80
  protocol        = "Tcp"
}

resource "azurerm_lb_rule" "http" {
  name                           = "http-rule"
  loadbalancer_id                = azurerm_lb.app.id
  frontend_ip_configuration_name = "PublicIPAddress"
  backend_address_pool_ids       = [azurerm_lb_backend_address_pool.app.id]
  probe_id                       = azurerm_lb_probe.http.id
  protocol                       = "Tcp"
  frontend_port                  = 80
  backend_port                   = 80
}

# Application Gateway
resource "azurerm_application_gateway" "main" {
  count               = var.enable_application_gateway ? 1 : 0
  name                = "${var.project_name}-appgw-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  location            = azurerm_resource_group.rg.location

  sku {
    name     = "Standard_v2"
    tier     = "Standard_v2"
    capacity = 2
  }

  gateway_ip_configuration {
    name      = "gateway-ip-config"
    subnet_id = azurerm_subnet.public[1].id
  }

  frontend_port {
    name = "http"
    port = 80
  }

  frontend_ip_configuration {
    name                 = "public"
    public_ip_address_id = azurerm_public_ip.appgw[0].id
  }

  backend_address_pool {
    name = "app-backend"
    ip_addresses = azurerm_linux_virtual_machine.app[*].private_ip_address
  }

  backend_http_settings {
    name                  = "http-settings"
    cookie_based_affinity = "Disabled"
    port                  = 80
    protocol              = "Http"
    request_timeout       = 60
  }

  http_listener {
    name                           = "http-listener"
    frontend_ip_configuration_name = "public"
    frontend_port_name             = "http"
    protocol                       = "Http"
  }

  request_routing_rule {
    name                       = "http-rule"
    rule_type                  = "Basic"
    http_listener_name         = "http-listener"
    backend_address_pool_name  = "app-backend"
    backend_http_settings_name = "http-settings"
    priority                   = 100
  }

  tags = local.tags
}

resource "azurerm_public_ip" "appgw" {
  count               = var.enable_application_gateway ? 1 : 0
  name                = "${var.project_name}-appgw-pip"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  allocation_method   = "Static"
  sku                 = "Standard"

  tags = local.tags
}

# Azure Front Door
resource "azurerm_cdn_frontdoor_profile" "main" {
  count               = var.enable_front_door ? 1 : 0
  name                = "${var.project_name}-fd-${var.environment}"
  resource_group_name = azurerm_resource_group.rg.name
  sku_name            = "Standard_AzureFrontDoor"

  tags = local.tags
}

resource "azurerm_cdn_frontdoor_endpoint" "main" {
  count                     = var.enable_front_door ? 1 : 0
  name                      = "${var.project_name}-fd-endpoint"
  cdn_frontdoor_profile_id  = azurerm_cdn_frontdoor_profile.main[0].id
}

resource "azurerm_cdn_frontdoor_origin_group" "main" {
  count                     = var.enable_front_door ? 1 : 0
  name                      = "${var.project_name}-fd-og"
  cdn_frontdoor_profile_id  = azurerm_cdn_frontdoor_profile.main[0].id
  session_affinity_enabled  = false

  health_probe {
    interval_in_seconds = 100
    path                = "/"
    protocol            = "Http"
    request_type        = "HEAD"
  }

  load_balancing {
    sample_size                        = 4
    successful_samples_required        = 2
    additional_latency_in_milliseconds = 0
  }
}

resource "azurerm_cdn_frontdoor_origin" "app" {
  count                     = var.enable_front_door ? 1 : 0
  name                      = "${var.project_name}-fd-origin"
  cdn_frontdoor_origin_group_id = azurerm_cdn_frontdoor_origin_group.main[0].id
  cdn_frontdoor_endpoint_id   = azurerm_cdn_frontdoor_endpoint.main[0].id

  host_name          = azurerm_public_ip.lb.ip_address
  http_port          = 80
  https_port         = 443
  origin_host_header = azurerm_public_ip.lb.ip_address
  priority           = 1
  weight             = 100
}
