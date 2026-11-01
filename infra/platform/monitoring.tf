# 7. Saved Log Analytics KQL Searches for Operational Correlation, Outbox Latency, and Failure Triage
resource "azurerm_log_analytics_saved_search" "correlation_trace" {
  name                       = "rc-correlation-trace"
  log_analytics_workspace_id = azurerm_log_analytics_workspace.main.id
  category                   = "ResourceCenter"
  display_name               = "End-to-End Correlation & Outbox-to-Cosmos Trace (Redacted PII)"
  query                      = <<-KQL
    ContainerAppConsoleLogs_CL
    | extend parsed = parse_json(Log_s)
    | where isnotnull(parsed.fields.correlation_id) or isnotnull(parsed.fields.event_id)
    | project
        TimeGenerated,
        Workload = coalesce(nonempty(ContainerAppName_s, ""), ContainerGroupName_s),
        Level = tostring(parsed.level),
        CorrelationId = tostring(parsed.fields.correlation_id),
        EventId = tostring(parsed.fields.event_id),
        RecipientRedacted = tostring(parsed.fields.recipient),
        Message = tostring(parsed.fields.message),
        DurationMs = toint(parsed.fields.duration_ms),
        AttemptCount = toint(parsed.fields.attempt_count)
    | order by TimeGenerated desc
  KQL
}

resource "azurerm_log_analytics_saved_search" "publisher_metrics" {
  name                       = "rc-outbox-publisher-metrics"
  log_analytics_workspace_id = azurerm_log_analytics_workspace.main.id
  category                   = "ResourceCenter"
  display_name               = "Loan Outbox Publisher Batch Duration & Backlog Metrics"
  query                      = <<-KQL
    ContainerAppConsoleLogs_CL
    | where Log_s has "outbox publisher batch finished"
    | extend parsed = parse_json(Log_s)
    | project
        TimeGenerated,
        ExecutionGroup = ContainerGroupName_s,
        ClaimedCount = toint(parsed.fields.claimed_count),
        PublishedCount = toint(parsed.fields.published_count),
        FailedCount = toint(parsed.fields.failed_count),
        DurationMs = toint(parsed.fields.duration_ms)
    | order by TimeGenerated desc
  KQL
}

resource "azurerm_log_analytics_saved_search" "service_errors" {
  name                       = "rc-service-warnings-and-errors"
  log_analytics_workspace_id = azurerm_log_analytics_workspace.main.id
  category                   = "ResourceCenter"
  display_name               = "Service Warnings, Retries, and Error Triage"
  query                      = <<-KQL
    ContainerAppConsoleLogs_CL
    | extend parsed = parse_json(Log_s)
    | where tostring(parsed.level) in ("WARN", "ERROR")
    | project
        TimeGenerated,
        Workload = coalesce(nonempty(ContainerAppName_s, ""), ContainerGroupName_s),
        Level = tostring(parsed.level),
        CorrelationId = tostring(parsed.fields.correlation_id),
        EventId = tostring(parsed.fields.event_id),
        ErrorCode = tostring(parsed.fields.error_code),
        ErrorDetail = tostring(parsed.fields.error),
        Message = tostring(parsed.fields.message)
    | order by TimeGenerated desc
  KQL
}
