variable "operator_object_id" { type = string }
resource "azurerm_resource_group" "state" {
  name     = "${local.prefix}-state-rg"
  location = var.location
  tags     = local.tags
}
resource "azurerm_storage_account" "state" {
  name                            = "${local.compact}state"
  resource_group_name             = azurerm_resource_group.state.name
  location                        = var.location
  account_tier                    = "Standard"
  account_replication_type        = "LRS"
  min_tls_version                 = "TLS1_2"
  shared_access_key_enabled       = false
  allow_nested_items_to_be_public = false
  default_to_oauth_authentication = true
  blob_properties {
    versioning_enabled = true
    delete_retention_policy { days = 30 }
    container_delete_retention_policy { days = 30 }
  }
  tags = local.tags
  lifecycle { prevent_destroy = true }
}
resource "azurerm_storage_container" "state" {
  for_each              = toset(["bootstrap", "platform", "workloads"])
  name                  = each.key
  storage_account_id    = azurerm_storage_account.state.id
  container_access_type = "private"
}
resource "azurerm_role_assignment" "operator_state" {
  for_each             = azurerm_storage_container.state
  scope                = "${azurerm_storage_account.state.id}/blobServices/default/containers/${each.key}"
  role_definition_name = "Storage Blob Data Contributor"
  principal_id         = var.operator_object_id
}
output "backend" {
  value = {
    resource_group_name  = azurerm_resource_group.state.name
    storage_account_name = azurerm_storage_account.state.name
    subscription_id      = var.subscription_id
    tenant_id            = var.tenant_id
    use_azuread_auth     = true
  }
}
