locals {
  servicebus_namespace_id = "/subscriptions/${var.handoff.subscription_id}/resourceGroups/${var.handoff.platform_resource_group}/providers/Microsoft.ServiceBus/namespaces/${local.servicebus_namespace}"
  postgres_server_name    = split(".", var.handoff.postgres_host)[0]
  postgres_server_id      = "/subscriptions/${var.handoff.subscription_id}/resourceGroups/${var.handoff.platform_resource_group}/providers/Microsoft.DBforPostgreSQL/flexibleServers/${local.postgres_server_name}"
}

# 1. Azure Monitor Action Group for operational alert routing
resource "azurerm_monitor_action_group" "ops" {
  name                = "${local.name_prefix}-ops-ag"
  resource_group_name = local.resource_group_name
  short_name          = "rcopsag"
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring" })

  arm_role_receiver {
    name                    = "subscription-owner"
    role_id                 = "8e3af657-a8ff-443c-a75c-2fe8c4bcb635"
    use_common_alert_schema = true
  }

  dynamic "email_receiver" {
    for_each = trimspace(var.alert_notification_email) != "" ? [trimspace(var.alert_notification_email)] : []
    content {
      name                    = "operator-email"
      email_address           = email_receiver.value
      use_common_alert_schema = true
    }
  }
}

# 2. Loan API 5xx Failure Rate Alert
resource "azurerm_monitor_metric_alert" "loan_api_5xx" {
  name                = "${local.name_prefix}-loan-api-5xx-alert"
  resource_group_name = local.resource_group_name
  scopes              = [azurerm_container_app.loan_api.id]
  description         = "Fires when the Loan HTTP API returns 5 or more HTTP 5xx server error responses within a 5-minute window."
  severity            = 2
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "loan" })

  criteria {
    metric_namespace = "Microsoft.App/containerApps"
    metric_name      = "Requests"
    aggregation      = "Total"
    operator         = "GreaterThanOrEqual"
    threshold        = 5

    dimension {
      name     = "statusCodeCategory"
      operator = "Include"
      values   = ["5xx"]
    }
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}

# 3. Service Bus Dead-Letter Queue Alert (Immediate Severity 1)
resource "azurerm_monitor_metric_alert" "servicebus_dlq" {
  name                = "${local.name_prefix}-servicebus-dlq-alert"
  resource_group_name = local.resource_group_name
  scopes              = [local.servicebus_namespace_id]
  description         = "Fires immediately when 1 or more notification messages enter the Service Bus dead-letter queue (DLQ)."
  severity            = 1
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "notifications" })

  criteria {
    metric_namespace = "Microsoft.ServiceBus/namespaces"
    metric_name      = "DeadletteredMessages"
    aggregation      = "Maximum"
    operator         = "GreaterThanOrEqual"
    threshold        = 1

    dimension {
      name     = "EntityName"
      operator = "Include"
      values   = [var.handoff.servicebus_queue]
    }
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}

# 4. Service Bus Queue Backlog / Oldest-Work Proxy Alert
resource "azurerm_monitor_metric_alert" "servicebus_backlog" {
  name                = "${local.name_prefix}-servicebus-backlog-alert"
  resource_group_name = local.resource_group_name
  scopes              = [local.servicebus_namespace_id]
  description         = "Fires when average active messages on notification-requests-v1 exceed 10 over 5 minutes (sustained queue backlog / oldest-work proxy)."
  severity            = 2
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "notifications" })

  criteria {
    metric_namespace = "Microsoft.ServiceBus/namespaces"
    metric_name      = "ActiveMessages"
    aggregation      = "Average"
    operator         = "GreaterThan"
    threshold        = 10

    dimension {
      name     = "EntityName"
      operator = "Include"
      values   = [var.handoff.servicebus_queue]
    }
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}

# 5. Loan Outbox Publisher Job Failure Alert
resource "azurerm_monitor_metric_alert" "job_failures" {
  name                = "${local.name_prefix}-job-failures-alert"
  resource_group_name = local.resource_group_name
  scopes              = [azurerm_container_app_job.publisher.id]
  description         = "Fires when the scheduled Loan outbox publisher job records a failed execution."
  severity            = 2
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "loan" })

  criteria {
    metric_namespace = "Microsoft.App/jobs"
    metric_name      = "Executions"
    aggregation      = "Total"
    operator         = "GreaterThanOrEqual"
    threshold        = 1

    dimension {
      name     = "state"
      operator = "Include"
      values   = ["Failed"]
    }
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}

# 6. PostgreSQL Flexible Server Connectivity & CPU Pressure Alert
resource "azurerm_monitor_metric_alert" "postgres_health" {
  name                = "${local.name_prefix}-postgres-health-alert"
  resource_group_name = local.resource_group_name
  scopes              = [local.postgres_server_id]
  description         = "Fires when the Loan PostgreSQL Flexible Server becomes unreachable (is_db_alive < 1) over a 5-minute window."
  severity            = 1
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "loan" })

  criteria {
    metric_namespace = "Microsoft.DBforPostgreSQL/flexibleServers"
    metric_name      = "is_db_alive"
    aggregation      = "Minimum"
    operator         = "LessThan"
    threshold        = 1
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}

resource "azurerm_monitor_metric_alert" "postgres_pressure" {
  name                = "${local.name_prefix}-postgres-pressure-alert"
  resource_group_name = local.resource_group_name
  scopes              = [local.postgres_server_id]
  description         = "Fires when Loan PostgreSQL CPU utilization exceeds 85% average over 5 minutes."
  severity            = 2
  frequency           = "PT1M"
  window_size         = "PT5M"
  auto_mitigate       = true
  enabled             = true
  tags                = merge(local.base_tags, { component = "monitoring", service = "loan" })

  criteria {
    metric_namespace = "Microsoft.DBforPostgreSQL/flexibleServers"
    metric_name      = "cpu_percent"
    aggregation      = "Average"
    operator         = "GreaterThan"
    threshold        = 85
  }

  action {
    action_group_id = azurerm_monitor_action_group.ops.id
  }
}
