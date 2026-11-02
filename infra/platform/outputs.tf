output "handoff" {
  description = "Explicit nonsensitive phase 2 interface; do not give workloads access to platform state."
  value = {
    subscription_id              = var.subscription_id
    tenant_id                    = var.tenant_id
    environment                  = var.environment
    location                     = var.location
    platform_resource_group      = azurerm_resource_group.platform.name
    workload_resource_group      = azurerm_resource_group.workloads.name
    container_app_environment_id = azurerm_container_app_environment.main.id
    registry_id                  = azurerm_container_registry.main.id
    registry_login_server        = azurerm_container_registry.main.login_server
    postgres_host                = azurerm_postgresql_flexible_server.loan.fqdn
    postgres_database            = azurerm_postgresql_flexible_server_database.loan.name
    cosmos_capacity_mode         = var.cosmos_capacity_mode
    registry_sku                 = var.registry_sku
    cosmos_endpoint              = azurerm_cosmosdb_account.notifications.endpoint
    cosmos_database              = azurerm_cosmosdb_sql_database.notifications.name
    cosmos_container             = azurerm_cosmosdb_sql_container.jobs.name
    servicebus_namespace         = "${azurerm_servicebus_namespace.main.name}.servicebus.windows.net"
    servicebus_queue             = azurerm_servicebus_queue.notifications.name
    notifications_vault_uri      = azurerm_key_vault.notifications.vault_uri
    web_vault_uri                = azurerm_key_vault.web.vault_uri
    github_client_id             = azurerm_user_assigned_identity.deploy.client_id
    identities = { for name, identity in azurerm_user_assigned_identity.workload : name => {
      id = identity.id, client_id = identity.client_id, principal_id = identity.principal_id, name = identity.name
    } }
  }
}
