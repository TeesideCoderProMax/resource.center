variable "postgres_admin_object_id" { type = string }
variable "postgres_admin_name" { type = string }
variable "postgres_admin_type" {
  type    = string
  default = "User"
}
resource "azurerm_postgresql_flexible_server" "loan" {
  name                          = "${local.prefix}-loan-pg"
  resource_group_name           = azurerm_resource_group.platform.name
  location                      = var.location
  version                       = "16"
  zone                          = var.postgres_zone
  sku_name                      = "B_Standard_B1ms"
  storage_mb                    = 32768
  backup_retention_days         = 7
  geo_redundant_backup_enabled  = false
  delegated_subnet_id           = azurerm_subnet.postgres.id
  private_dns_zone_id           = azurerm_private_dns_zone.postgres.id
  public_network_access_enabled = false
  authentication {
    active_directory_auth_enabled = true
    password_auth_enabled         = false
    tenant_id                     = var.tenant_id
  }
  tags       = local.tags
  depends_on = [azurerm_private_dns_zone_virtual_network_link.postgres]
  lifecycle { prevent_destroy = true }
}
resource "azurerm_postgresql_flexible_server_active_directory_administrator" "operator" {
  server_name         = azurerm_postgresql_flexible_server.loan.name
  resource_group_name = azurerm_resource_group.platform.name
  tenant_id           = var.tenant_id
  object_id           = var.postgres_admin_object_id
  principal_name      = var.postgres_admin_name
  principal_type      = var.postgres_admin_type
}
resource "azurerm_postgresql_flexible_server_database" "loan" {
  name      = "loan"
  server_id = azurerm_postgresql_flexible_server.loan.id
  charset   = "UTF8"
  collation = "en_US.utf8"
  lifecycle { prevent_destroy = true }
}
resource "azurerm_cosmosdb_account" "notifications" {
  name                         = "${local.prefix}-notifications"
  resource_group_name          = azurerm_resource_group.platform.name
  location                     = var.location
  offer_type                   = "Standard"
  kind                         = "GlobalDocumentDB"
  local_authentication_enabled = false
  free_tier_enabled            = var.cosmos_capacity_mode == "free_tier"
  dynamic "capabilities" {
    for_each = var.cosmos_capacity_mode == "serverless" ? [1] : []
    content { name = "EnableServerless" }
  }
  dynamic "capacity" {
    for_each = var.cosmos_capacity_mode == "free_tier" ? [1] : []
    content { total_throughput_limit = 1000 }
  }
  consistency_policy { consistency_level = "Session" }
  geo_location {
    location          = var.location
    failover_priority = 0
  }
  backup {
    type                = "Periodic"
    interval_in_minutes = 240
    retention_in_hours  = 24
    storage_redundancy  = "Local"
  }
  tags = local.tags
  lifecycle { prevent_destroy = true }
}
resource "azurerm_cosmosdb_sql_database" "notifications" {
  throughput          = var.cosmos_capacity_mode == "free_tier" ? 400 : null
  name                = "notifications"
  resource_group_name = azurerm_resource_group.platform.name
  account_name        = azurerm_cosmosdb_account.notifications.name
}
resource "azurerm_cosmosdb_sql_container" "jobs" {
  name                  = "jobs"
  resource_group_name   = azurerm_resource_group.platform.name
  account_name          = azurerm_cosmosdb_account.notifications.name
  database_name         = azurerm_cosmosdb_sql_database.notifications.name
  partition_key_paths   = ["/eventId"]
  partition_key_version = 2
  default_ttl           = -1
  lifecycle { prevent_destroy = true }
}
resource "azurerm_servicebus_namespace" "main" {
  name                = "${local.prefix}-bus"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  sku                 = "Basic"
  local_auth_enabled  = false
  minimum_tls_version = "1.2"
  tags                = local.tags
}
resource "azurerm_servicebus_queue" "notifications" {
  name                                 = "notification-requests-v1"
  namespace_id                         = azurerm_servicebus_namespace.main.id
  max_size_in_megabytes                = 1024
  default_message_ttl                  = "P14D"
  lock_duration                        = "PT1M"
  max_delivery_count                   = 10
  dead_lettering_on_message_expiration = true
}
