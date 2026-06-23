# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — ExpressRoute, VPN Gateway, Private Link
# ---------------------------------------------------------------------------

# VPN Gateway (Site-to-Site)
resource "azurerm_virtual_network_gateway" "vpn" {
  count               = var.enable_vpn_gateway ? 1 : 0
  name                = "${var.project_name}-vpn-gw-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name

  type     = "Vpn"
  vpn_type = "RouteBased"
  sku      = "VpnGw1"

  active_active = false
  enable_bgp    = false

  ip_configuration {
    name                          = "vnetGatewayConfig"
    public_ip_address_id          = azurerm_public_ip.vpn[0].id
    private_ip_address_allocation = "Dynamic"
    subnet_id                     = azurerm_subnet_gateway.id
  }

  tags = local.tags
}

resource "azurerm_public_ip" "vpn" {
  count               = var.enable_vpn_gateway ? 1 : 0
  name                = "${var.project_name}-vpn-pip"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  allocation_method   = "Static"
  sku                 = "Standard"

  tags = local.tags
}

resource "azurerm_subnet" "gateway" {
  count                = var.enable_vpn_gateway || var.enable_expressroute ? 1 : 0
  name                 = "GatewaySubnet"
  resource_group_name  = azurerm_resource_group.rg.name
  virtual_network_name = azurerm_virtual_network.vnet.name
  address_prefixes     = ["10.1.255.0/27"]
}

# ExpressRoute Gateway
resource "azurerm_virtual_network_gateway" "expressroute" {
  count               = var.enable_expressroute ? 1 : 0
  name                = "${var.project_name}-er-gw-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name

  type     = "ExpressRoute"
  sku      = "Standard"

  ip_configuration {
    name                          = "erGatewayConfig"
    public_ip_address_id          = azurerm_public_ip.expressroute[0].id
    private_ip_address_allocation = "Dynamic"
    subnet_id                     = azurerm_subnet.gateway[0].id
  }

  tags = local.tags
}

# Local Network Gateway (on-premises endpoint)
resource "azurerm_local_network_gateway" "onprem" {
  count               = var.enable_vpn_gateway ? 1 : 0
  name                = "${var.project_name}-lng-onprem-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  gateway_address     = var.onprem_gateway_ip
  address_space       = var.onprem_address_spaces

  tags = local.tags
}

# Site-to-Site VPN Connection
resource "azurerm_virtual_network_gateway_connection" "s2s" {
  count               = var.enable_vpn_gateway ? 1 : 0
  name                = "${var.project_name}-vpn-conn-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name

  type                       = "IPsec"
  virtual_network_gateway_id = azurerm_virtual_network_gateway.vpn[0].id
  local_network_gateway_id   = azurerm_local_network_gateway.onprem[0].id
  shared_key                 = var.vpn_shared_key

  ipsec_policy {
    ike_encryption   = "AES256"
    ike_integrity    = "SHA256"
    dh_group         = "DHGroup14"
    ipsec_encryption = "AES256"
    ipsec_integrity  = "SHA256"
    pfs_group        = "PFS14"
    sa_lifetime      = 3600
  }

  tags = local.tags
}

resource "azurerm_public_ip" "expressroute" {
  count               = var.enable_expressroute ? 1 : 0
  name                = "${var.project_name}-er-pip"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  allocation_method   = "Static"
  sku                 = "Standard"

  tags = local.tags
}

# Private Link pour SQL Server
resource "azurerm_private_endpoint" "sql" {
  count               = var.enable_private_link ? 1 : 0
  name                = "${var.project_name}-sql-pe-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  subnet_id           = azurerm_subnet.private[0].id

  private_service_connection {
    name                           = "sql-privateserviceconnection"
    private_connection_resource_id = azurerm_mssql_server.main.id
    subresource_names              = ["sqlServer"]
    is_manual_connection           = false
  }

  tags = local.tags
}

resource "azurerm_private_dns_zone" "sql" {
  count               = var.enable_private_link ? 1 : 0
  name                = "privatelink.database.windows.net"
  resource_group_name = azurerm_resource_group.rg.name
}

resource "azurerm_private_dns_zone_virtual_network_link" "sql" {
  count                 = var.enable_private_link ? 1 : 0
  name                  = "sql-vnet-link"
  resource_group_name   = azurerm_resource_group.rg.name
  private_dns_zone_name = azurerm_private_dns_zone.sql[0].name
  virtual_network_id    = azurerm_virtual_network.vnet.id
}

resource "azurerm_private_dns_a_record" "sql" {
  count               = var.enable_private_link ? 1 : 0
  name                = azurerm_mssql_server.main.name
  zone_name           = azurerm_private_dns_zone.sql[0].name
  resource_group_name = azurerm_resource_group.rg.name
  ttl                 = 300
  records             = [azurerm_private_endpoint.sql[0].private_service_connection[0].private_ip_address]
}
