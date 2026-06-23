locals {
  tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "terraform"
  }
  # Nom de compte de stockage : 3-24 caracteres, minuscules et chiffres uniquement.
  storage_account_name = lower(substr(replace("${var.project_name}backups${var.environment}", "-", ""), 0, 24))
}

# ---------------------------------------------------------------------------
# Gouvernance : Resource Group
# ---------------------------------------------------------------------------
resource "azurerm_resource_group" "rg" {
  name     = "${var.project_name}-${var.environment}-rg"
  location = var.azure_region
  tags     = local.tags
}

# ---------------------------------------------------------------------------
# Réseau : VNet, subnets, NSG (Azure Solutions Architect : segmentation)
# ---------------------------------------------------------------------------
resource "azurerm_virtual_network" "vnet" {
  name                = "${var.project_name}-vnet"
  address_space       = [var.vnet_cidr]
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  tags                = local.tags
}

resource "azurerm_subnet" "public" {
  count                = length(var.public_subnet_cidrs)
  name                 = "public-${count.index + 1}"
  resource_group_name  = azurerm_resource_group.rg.name
  virtual_network_name = azurerm_virtual_network.vnet.name
  address_prefixes       = [var.public_subnet_cidrs[count.index]]
}

resource "azurerm_subnet" "private" {
  count                = length(var.private_subnet_cidrs)
  name                 = "private-${count.index + 1}"
  resource_group_name  = azurerm_resource_group.rg.name
  virtual_network_name = azurerm_virtual_network.vnet.name
  address_prefixes       = [var.private_subnet_cidrs[count.index]]
}

resource "azurerm_network_security_group" "bastion" {
  name                = "${var.project_name}-bastion-nsg"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  tags                = local.tags

  security_rule {
    name                       = "AllowSSHAdmin"
    priority                   = 100
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "22"
    source_address_prefix      = var.allowed_ssh_cidr
    destination_address_prefix = "*"
  }
}

resource "azurerm_network_security_group" "app" {
  name                = "${var.project_name}-app-nsg"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  tags                = local.tags

  security_rule {
    name                       = "AllowHTTP"
    priority                   = 100
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "80"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  security_rule {
    name                       = "AllowHTTPS"
    priority                   = 110
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "443"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  security_rule {
    name                       = "AllowSSHFromBastion"
    priority                   = 120
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "22"
    source_address_prefix      = azurerm_subnet.public[0].address_prefixes[0]
    destination_address_prefix = "*"
  }
}

resource "azurerm_subnet_network_security_group_association" "public" {
  count                     = length(azurerm_subnet.public)
  subnet_id                 = azurerm_subnet.public[count.index].id
  network_security_group_id = azurerm_network_security_group.bastion.id
}

resource "azurerm_subnet_network_security_group_association" "private" {
  count                     = length(azurerm_subnet.private)
  subnet_id                 = azurerm_subnet.private[count.index].id
  network_security_group_id = azurerm_network_security_group.app.id
}

# ---------------------------------------------------------------------------
# IP publique + Bastion Azure
# ---------------------------------------------------------------------------
resource "azurerm_public_ip" "bastion" {
  name                = "${var.project_name}-bastion-pip"
  resource_group_name = azurerm_resource_group.rg.name
  location            = azurerm_resource_group.rg.location
  allocation_method   = "Static"
  sku                 = "Standard"
  tags                = local.tags
}

resource "azurerm_network_interface" "bastion" {
  name                = "${var.project_name}-bastion-nic"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  tags                = local.tags

  ip_configuration {
    name                          = "internal"
    subnet_id                     = azurerm_subnet.public[0].id
    private_ip_address_allocation = "Dynamic"
    public_ip_address_id          = azurerm_public_ip.bastion.id
  }
}

resource "azurerm_linux_virtual_machine" "bastion" {
  name                            = "${var.project_name}-bastion"
  resource_group_name             = azurerm_resource_group.rg.name
  location                        = azurerm_resource_group.rg.location
  size                            = var.vm_size_bastion
  admin_username                  = var.admin_username
  network_interface_ids           = [azurerm_network_interface.bastion.id]
  disable_password_authentication = true
  tags                            = local.tags

  admin_ssh_key {
    username   = var.admin_username
    public_key = file("${path.module}/id_rsa.pub")
  }

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "StandardSSD_LRS"
    disk_size_gb         = 30
  }

  source_image_reference {
    publisher = "Canonical"
    offer     = "0001-com-ubuntu-server-jammy"
    sku       = "22_04-lts-gen2"
    version   = "latest"
  }

  identity {
    type = "SystemAssigned"
  }

  custom_data = base64encode(file("${path.module}/cloud-init.sh"))
}

# ---------------------------------------------------------------------------
# VMs applicatives (private subnet)
# ---------------------------------------------------------------------------
resource "azurerm_network_interface" "app" {
  count               = 2
  name                = "${var.project_name}-app-nic-${count.index + 1}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  tags                = local.tags

  ip_configuration {
    name                          = "internal"
    subnet_id                     = azurerm_subnet.private[count.index].id
    private_ip_address_allocation = "Dynamic"
  }
}

resource "azurerm_linux_virtual_machine" "app" {
  count                           = 2
  name                            = "${var.project_name}-app-${count.index + 1}"
  resource_group_name             = azurerm_resource_group.rg.name
  location                        = azurerm_resource_group.rg.location
  size                            = var.vm_size_app
  admin_username                  = var.admin_username
  network_interface_ids           = [azurerm_network_interface.app[count.index].id]
  disable_password_authentication = true
  tags                            = local.tags

  admin_ssh_key {
    username   = var.admin_username
    public_key = file("${path.module}/id_rsa.pub")
  }

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "StandardSSD_LRS"
    disk_size_gb         = 40
  }

  source_image_reference {
    publisher = "Canonical"
    offer     = "0001-com-ubuntu-server-jammy"
    sku       = "22_04-lts-gen2"
    version   = "latest"
  }

  identity {
    type = "SystemAssigned"
  }

  custom_data = base64encode(file("${path.module}/cloud-init.sh"))
}

# ---------------------------------------------------------------------------
# Azure Monitor & Log Analytics (observabilite cloud)
# ---------------------------------------------------------------------------
resource "azurerm_log_analytics_workspace" "monitor" {
  name                = "${var.project_name}-logs-${var.environment}"
  location            = azurerm_resource_group.rg.location
  resource_group_name = azurerm_resource_group.rg.name
  sku                 = "PerGB2018"
  retention_in_days   = 30
  tags                = local.tags
}

resource "azurerm_monitor_diagnostic_setting" "vm_diag" {
  count                      = length(azurerm_linux_virtual_machine.app)
  name                       = "vm-diagnostics-${count.index + 1}"
  target_resource_id         = azurerm_linux_virtual_machine.app[count.index].id
  log_analytics_workspace_id = azurerm_log_analytics_workspace.monitor.id

  metric {
    category = "AllMetrics"
  }
}

# ---------------------------------------------------------------------------
# Stockage de backups (chiffre, private access)
# ---------------------------------------------------------------------------
resource "azurerm_storage_account" "backups" {
  name                     = local.storage_account_name
  resource_group_name      = azurerm_resource_group.rg.name
  location                 = azurerm_resource_group.rg.location
  account_tier             = "Standard"
  account_replication_type = "LRS"
  min_tls_version          = "TLS1_2"
  allow_nested_items_to_be_public = false
  tags                     = local.tags
}

resource "azurerm_storage_container" "backups" {
  name                  = "backups"
  storage_account_name  = azurerm_storage_account.backups.name
  container_access_type = "private"
}
