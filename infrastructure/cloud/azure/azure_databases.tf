# ---------------------------------------------------------------------------
# Azure Solutions Architect Expert — SQL Database, Cosmos DB, Data Lake
# ---------------------------------------------------------------------------

# SQL Server
resource "azurerm_mssql_server" "main" {
  name                         = "${var.project_name}-sql-${var.environment}"
  resource_group_name          = azurerm_resource_group.rg.name
  location                     = azurerm_resource_group.rg.location
  version                      = "12.0"
  administrator_login          = "sqladmin"
  administrator_login_password = var.sql_admin_password
  minimum_tls_version          = "1.2"

  azuread_administrator {
    login_username = "AzureAD Admin"
    object_id      = data.azuread_client_config.current.object_id
  }

  tags = local.tags
}

# SQL Database
resource "azurerm_mssql_database" "main" {
  name         = "appdb"
  server_id    = azurerm_mssql_server.main.id
  sku_name     = "S0"
  collation    = "SQL_Latin1_General_CP1_CI_AS"
  max_size_gb  = 2

  tags = local.tags
}

# Firewall SQL : autorise le VNet uniquement
resource "azurerm_mssql_firewall_rule" "azure_services" {
  name             = "AllowAzureServices"
  server_id        = azurerm_mssql_server.main.id
  start_ip_address = "0.0.0.0"
  end_ip_address   = "0.0.0.0"
}

resource "azurerm_mssql_virtual_network_rule" "main" {
  name      = "sql-vnet-rule"
  server_id = azurerm_mssql_server.main.id
  subnet_id = azurerm_subnet.private[0].id
}

# Cosmos DB account
resource "azurerm_cosmosdb_account" "main" {
  name                = "${var.project_name}-cosmos-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  offer_type          = "Standard"
  kind                = "GlobalDocumentDB"

  enable_free_tier = true

  geo_location {
    location          = var.azure_region
    failover_priority = 0
  }

  consistency_policy {
    consistency_level = "Session"
  }

  capabilities {
    name = "EnableServerless"
  }

  tags = local.tags
}

resource "azurerm_cosmosdb_sql_database" "main" {
  name                = "appdata"
  resource_group_name = azurerm_resource_group.rg.name
  account_name        = azurerm_cosmosdb_account.main.name
}

# Data Lake Gen2 (Storage Account avec Hierarchical Namespace)
resource "azurerm_storage_account" "datalake" {
  name                     = lower(substr(replace("${var.project_name}datalake${var.environment}", "-", ""), 0, 24))
  resource_group_name      = azurerm_resource_group.rg.name
  location                 = azurerm_resource_group.rg.location
  account_tier             = "Standard"
  account_replication_type = "LRS"
  is_hns_enabled           = true
  min_tls_version          = "TLS1_2"

  tags = local.tags
}

resource "azurerm_storage_data_lake_gen2_filesystem" "raw" {
  name               = "raw"
  storage_account_id = azurerm_storage_account.datalake.id
}
