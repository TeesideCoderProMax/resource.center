variable "github_repository" { type = string }
# GitHub sends ID-based OIDC subjects for this repository (use_immutable_subject), e.g.
# "repo:Owner@123/name@456". Empty keeps only the name-based credential.
variable "github_immutable_subject_prefix" {
  type    = string
  default = ""
}
variable "state_storage_account_id" { type = string }
resource "azurerm_user_assigned_identity" "workload" {
  for_each            = toset(["loan", "publisher", "notifications", "migrations", "web"])
  name                = "${local.prefix}-${each.key}"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  tags                = local.tags
}
resource "azurerm_role_assignment" "image_pull" {
  for_each             = azurerm_user_assigned_identity.workload
  scope                = azurerm_container_registry.main.id
  role_definition_name = "AcrPull"
  principal_id         = each.value.principal_id
}
resource "azurerm_role_assignment" "send" {
  scope                = azurerm_servicebus_queue.notifications.id
  role_definition_name = "Azure Service Bus Data Sender"
  principal_id         = azurerm_user_assigned_identity.workload["publisher"].principal_id
}
resource "azurerm_role_assignment" "receive" {
  scope                = azurerm_servicebus_queue.notifications.id
  role_definition_name = "Azure Service Bus Data Receiver"
  principal_id         = azurerm_user_assigned_identity.workload["notifications"].principal_id
}
resource "azurerm_cosmosdb_sql_role_assignment" "notifications" {
  resource_group_name = azurerm_resource_group.platform.name
  account_name        = azurerm_cosmosdb_account.notifications.name
  role_definition_id  = "${azurerm_cosmosdb_account.notifications.id}/sqlRoleDefinitions/00000000-0000-0000-0000-000000000002"
  principal_id        = azurerm_user_assigned_identity.workload["notifications"].principal_id
  scope               = "${azurerm_cosmosdb_account.notifications.id}/dbs/${azurerm_cosmosdb_sql_database.notifications.name}/colls/${azurerm_cosmosdb_sql_container.jobs.name}"
}
resource "azurerm_role_assignment" "web_secrets" {
  scope                = azurerm_key_vault.web.id
  role_definition_name = "Key Vault Secrets User"
  principal_id         = azurerm_user_assigned_identity.workload["web"].principal_id
}
# The operator writes the web secret values out of band (never Terraform), so they never enter state.
variable "web_secrets_operator_object_id" {
  type    = string
  default = ""
}
resource "azurerm_role_assignment" "web_secrets_operator" {
  scope                = azurerm_key_vault.web.id
  role_definition_name = "Key Vault Secrets Officer"
  principal_id         = coalesce(var.web_secrets_operator_object_id, var.postgres_admin_object_id)
}
resource "azurerm_role_assignment" "notification_secrets" {
  scope                = azurerm_key_vault.notifications.id
  role_definition_name = "Key Vault Secrets User"
  principal_id         = azurerm_user_assigned_identity.workload["notifications"].principal_id
}
resource "azurerm_user_assigned_identity" "deploy" {
  name                = "${local.prefix}-github-workloads"
  resource_group_name = azurerm_resource_group.platform.name
  location            = var.location
  tags                = local.tags
}
resource "azurerm_federated_identity_credential" "github" {
  name                      = "github-environment"
  user_assigned_identity_id = azurerm_user_assigned_identity.deploy.id
  audience                  = ["api://AzureADTokenExchange"]
  issuer                    = "https://token.actions.githubusercontent.com"
  subject                   = "repo:${var.github_repository}:environment:${var.environment}"
}
resource "azurerm_federated_identity_credential" "github_immutable" {
  count                     = var.github_immutable_subject_prefix == "" ? 0 : 1
  name                      = "github-environment-immutable"
  user_assigned_identity_id = azurerm_user_assigned_identity.deploy.id
  audience                  = ["api://AzureADTokenExchange"]
  issuer                    = "https://token.actions.githubusercontent.com"
  subject                   = "${var.github_immutable_subject_prefix}:environment:${var.environment}"
}
resource "azurerm_role_assignment" "deploy_workloads" {
  scope                = azurerm_resource_group.workloads.id
  role_definition_name = "Contributor"
  principal_id         = azurerm_user_assigned_identity.deploy.principal_id
}
resource "azurerm_role_assignment" "deploy_read" {
  scope                = azurerm_resource_group.platform.id
  role_definition_name = "Reader"
  principal_id         = azurerm_user_assigned_identity.deploy.principal_id
}
resource "azurerm_role_assignment" "deploy_images" {
  scope                = azurerm_container_registry.main.id
  role_definition_name = "AcrPush"
  principal_id         = azurerm_user_assigned_identity.deploy.principal_id
}
resource "azurerm_role_assignment" "deploy_identity" {
  for_each             = azurerm_user_assigned_identity.workload
  scope                = each.value.id
  role_definition_name = "Managed Identity Operator"
  principal_id         = azurerm_user_assigned_identity.deploy.principal_id
}
resource "azurerm_role_assignment" "deploy_environment" {
  scope              = azurerm_container_app_environment.main.id
  role_definition_id = azurerm_role_definition.environment_join.role_definition_resource_id
  principal_id       = azurerm_user_assigned_identity.deploy.principal_id
}
resource "azurerm_role_assignment" "deploy_state" {
  scope                = "${var.state_storage_account_id}/blobServices/default/containers/workloads"
  role_definition_name = "Storage Blob Data Contributor"
  principal_id         = azurerm_user_assigned_identity.deploy.principal_id
}

resource "azurerm_role_definition" "environment_join" {
  name        = "${local.prefix}-environment-join"
  scope       = azurerm_resource_group.platform.id
  description = "Attach workloads to the platform environment without managing the environment."
  permissions {
    actions = ["Microsoft.App/managedEnvironments/read", "Microsoft.App/managedEnvironments/join/action"]
  }
  assignable_scopes = [azurerm_resource_group.platform.id]
}
