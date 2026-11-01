output "environment" {
  description = "Deployed workload environment."
  value       = var.environment
}

output "resource_group_name" {
  description = "Workloads resource group name."
  value       = local.resource_group_name
}

output "loan_api_name" {
  description = "Loan API Container App name."
  value       = azurerm_container_app.loan_api.name
}

output "loan_api_fqdn" {
  description = "Loan API Container App managed HTTPS FQDN."
  value       = azurerm_container_app.loan_api.ingress[0].fqdn
}

output "loan_api_url" {
  description = "Loan API HTTPS base URL."
  value       = "https://${azurerm_container_app.loan_api.ingress[0].fqdn}"
}

output "web_app_name" {
  description = "Next.js Web Frontend Container App name."
  value       = azurerm_container_app.web.name
}

output "web_app_fqdn" {
  description = "Next.js Web Frontend Container App managed HTTPS FQDN."
  value       = azurerm_container_app.web.ingress[0].fqdn
}

output "web_app_url" {
  description = "Next.js Web Frontend HTTPS URL."
  value       = "https://${azurerm_container_app.web.ingress[0].fqdn}"
}

output "loan_migrations_job_name" {
  description = "Loan PostgreSQL migrations Container Apps Job name."
  value       = azurerm_container_app_job.migrations.name
}

output "loan_publisher_job_name" {
  description = "Loan outbox publisher Container Apps Job name."
  value       = azurerm_container_app_job.publisher.name
}

output "notifications_worker_name" {
  description = "Notifications worker Container App name (no HTTP ingress)."
  value       = azurerm_container_app.notifications.name
}

output "backend_image" {
  description = "Immutable digest-pinned backend image deployed."
  value       = var.backend_image
}

output "web_image" {
  description = "Immutable digest-pinned frontend image deployed."
  value       = var.web_image
}

output "ops_action_group_id" {
  description = "Azure Monitor Action Group ID for operational alerts."
  value       = azurerm_monitor_action_group.ops.id
}

output "metric_alert_ids" {
  description = "Provisioned Azure Monitor metric alert resource IDs."
  value = {
    loan_api_5xx       = azurerm_monitor_metric_alert.loan_api_5xx.id
    servicebus_dlq     = azurerm_monitor_metric_alert.servicebus_dlq.id
    servicebus_backlog = azurerm_monitor_metric_alert.servicebus_backlog.id
    job_failures       = azurerm_monitor_metric_alert.job_failures.id
    postgres_health    = azurerm_monitor_metric_alert.postgres_health.id
    postgres_pressure  = azurerm_monitor_metric_alert.postgres_pressure.id
  }
}
