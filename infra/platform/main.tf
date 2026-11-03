resource "azurerm_resource_group" "platform" {
  name     = "${local.prefix}-platform-rg"
  location = var.location
  tags     = local.tags
}
resource "azurerm_resource_group" "workloads" {
  name     = "${local.prefix}-workloads-rg"
  location = var.location
  tags     = local.tags
}
resource "azurerm_virtual_network" "main" {
  name                = "${local.prefix}-vnet"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  address_space       = ["10.42.0.0/16"]
  tags                = local.tags
}
resource "azurerm_subnet" "apps" {
  name                 = "container-apps"
  resource_group_name  = azurerm_resource_group.platform.name
  virtual_network_name = azurerm_virtual_network.main.name
  address_prefixes     = ["10.42.0.0/23"]
  delegation {
    name = "container-apps"
    service_delegation {
      name    = "Microsoft.App/environments"
      actions = ["Microsoft.Network/virtualNetworks/subnets/join/action"]
    }
  }
}
resource "azurerm_subnet" "postgres" {
  name                 = "postgres"
  resource_group_name  = azurerm_resource_group.platform.name
  virtual_network_name = azurerm_virtual_network.main.name
  address_prefixes     = ["10.42.2.0/24"]
  service_endpoints    = ["Microsoft.Storage"]
  delegation {
    name = "postgres"
    service_delegation {
      name    = "Microsoft.DBforPostgreSQL/flexibleServers"
      actions = ["Microsoft.Network/virtualNetworks/subnets/join/action"]
    }
  }
}
resource "azurerm_private_dns_zone" "postgres" {
  name                = "${local.prefix}.postgres.database.azure.com"
  resource_group_name = azurerm_resource_group.platform.name
  tags                = local.tags
}
resource "azurerm_private_dns_zone_virtual_network_link" "postgres" {
  name                  = "postgres"
  resource_group_name   = azurerm_resource_group.platform.name
  private_dns_zone_name = azurerm_private_dns_zone.postgres.name
  virtual_network_id    = azurerm_virtual_network.main.id
}
resource "azurerm_log_analytics_workspace" "main" {
  name                = "${local.prefix}-logs"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  sku                 = "PerGB2018"
  retention_in_days   = 30
  daily_quota_gb      = 1
  tags                = local.tags
}
resource "azurerm_container_app_environment" "main" {
  name                       = "${local.prefix}-apps"
  resource_group_name        = azurerm_resource_group.platform.name
  location                   = var.location
  log_analytics_workspace_id = azurerm_log_analytics_workspace.main.id
  infrastructure_subnet_id   = azurerm_subnet.apps.id
  workload_profile {
    name                  = "Consumption"
    workload_profile_type = "Consumption"
  }
  tags = local.tags
}
resource "azurerm_container_registry" "main" {
  name                = "${local.compact}acr"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  sku                 = var.registry_sku
  admin_enabled       = false
  tags                = local.tags
}
# Web-owned vault for the server-side Auth0 client secret and session secret.
resource "azurerm_key_vault" "web" {
  name                       = "${local.compact}wkv"
  resource_group_name        = azurerm_resource_group.platform.name
  location                   = var.location
  tenant_id                  = var.tenant_id
  sku_name                   = "standard"
  rbac_authorization_enabled = true
  purge_protection_enabled   = true
  soft_delete_retention_days = 30
  tags                       = local.tags
}
resource "azurerm_key_vault" "notifications" {
  name                       = "${local.compact}nkv"
  resource_group_name        = azurerm_resource_group.platform.name
  location                   = var.location
  tenant_id                  = var.tenant_id
  sku_name                   = "standard"
  rbac_authorization_enabled = true
  purge_protection_enabled   = true
  soft_delete_retention_days = 30
  tags                       = local.tags
}
