locals {
  name_prefix            = var.name_prefix
  resource_group_name    = var.handoff.workload_resource_group
  servicebus_namespace   = replace(var.handoff.servicebus_namespace, ".servicebus.windows.net", "")
  loan_api_name          = "${local.name_prefix}-loan-api"
  loan_publisher_name    = "${local.name_prefix}-loan-publisher"
  loan_migrations_name   = "${local.name_prefix}-migrations"
  notifications_app_name = "${local.name_prefix}-notifications"
  web_app_name           = "${local.name_prefix}-web"

  base_tags = merge(
    {
      project     = "resource-center"
      environment = var.environment
      managed_by  = "terraform"
      root_module = "workloads"
    },
    var.tags
  )
}

# 1. Loan API Container App (Rust/Tokio/Axum, external HTTPS ingress, scale-to-zero)
resource "azurerm_container_app" "loan_api" {
  name                         = local.loan_api_name
  container_app_environment_id = var.handoff.container_app_environment_id
  resource_group_name          = local.resource_group_name
  revision_mode                = "Single"
  workload_profile_name        = "Consumption"
  tags                         = merge(local.base_tags, { service = "loan" })

  identity {
    type         = "UserAssigned"
    identity_ids = [var.handoff.identities["loan"].id]
  }

  registry {
    server   = var.handoff.registry_login_server
    identity = var.handoff.identities["loan"].id
  }

  ingress {
    external_enabled           = true
    target_port                = 8080
    transport                  = "auto"
    allow_insecure_connections = false

    traffic_weight {
      percentage      = 100
      latest_revision = true
    }
  }

  template {
    min_replicas = 0
    max_replicas = 3

    http_scale_rule {
      name                = "http-concurrency"
      concurrent_requests = "20"
    }

    container {
      name    = "loan-api"
      image   = var.backend_image
      cpu     = 0.25
      memory  = "0.5Gi"
      command = ["loan-service", "api"]

      env {
        name  = "PORT"
        value = "8080"
      }
      env {
        name  = "RC_ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "RC_VERSION"
        value = var.service_version
      }
      env {
        name  = "SERVICE_VERSION"
        value = var.service_version
      }
      env {
        name  = "AZURE_CLIENT_ID"
        value = var.handoff.identities["loan"].client_id
      }
      env {
        name  = "PGHOST"
        value = var.handoff.postgres_host
      }
      env {
        name  = "PGPORT"
        value = "5432"
      }
      env {
        name  = "PGDATABASE"
        value = var.handoff.postgres_database
      }
      env {
        name  = "PGUSER"
        value = var.handoff.identities["loan"].name
      }
      env {
        name  = "PG_MAX_CONNECTIONS"
        value = "5"
      }
      env {
        name  = "AUTH0_DOMAIN"
        value = var.auth0_domain
      }
      env {
        name  = "AUTH0_AUDIENCE"
        value = var.auth0_audience
      }
      env {
        name  = "CORS_ALLOWED_ORIGINS"
        value = "https://${local.web_app_name}.livelywave-2cc0564d.francecentral.azurecontainerapps.io"
      }

      startup_probe {
        transport               = "HTTP"
        port                    = 8080
        path                    = "/health/startup"
        initial_delay           = 2
        interval_seconds        = 5
        timeout                 = 3
        failure_count_threshold = 12
      }

      liveness_probe {
        transport               = "HTTP"
        port                    = 8080
        path                    = "/health/live"
        initial_delay           = 5
        interval_seconds        = 15
        timeout                 = 3
        failure_count_threshold = 3
      }

      readiness_probe {
        transport               = "HTTP"
        port                    = 8080
        path                    = "/health/ready"
        initial_delay           = 3
        interval_seconds        = 10
        timeout                 = 5
        failure_count_threshold = 3
      }
    }
  }
}

# 2. Next.js Web Frontend Container App (Consumption, external HTTPS ingress, scale-to-zero)
resource "azurerm_container_app" "web" {
  name                         = local.web_app_name
  container_app_environment_id = var.handoff.container_app_environment_id
  resource_group_name          = local.resource_group_name
  revision_mode                = "Single"
  workload_profile_name        = "Consumption"
  tags                         = merge(local.base_tags, { service = "web" })

  identity {
    type         = "UserAssigned"
    identity_ids = [var.handoff.identities["loan"].id]
  }

  registry {
    server   = var.handoff.registry_login_server
    identity = var.handoff.identities["loan"].id
  }

  ingress {
    external_enabled           = true
    target_port                = 3000
    transport                  = "auto"
    allow_insecure_connections = false

    traffic_weight {
      percentage      = 100
      latest_revision = true
    }
  }

  template {
    min_replicas = 0
    max_replicas = 3

    http_scale_rule {
      name                = "http-concurrency"
      concurrent_requests = "20"
    }

    container {
      name   = "web"
      image  = var.web_image
      cpu    = 0.25
      memory = "0.5Gi"

      env {
        name  = "PORT"
        value = "3000"
      }
      env {
        name  = "HOSTNAME"
        value = "0.0.0.0"
      }
      env {
        name  = "RC_ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "LOAN_API_BASE_URL"
        value = "https://${azurerm_container_app.loan_api.ingress[0].fqdn}"
      }
      env {
        name  = "NEXT_PUBLIC_LOAN_API_URL"
        value = "https://${azurerm_container_app.loan_api.ingress[0].fqdn}"
      }
      env {
        name  = "NEXT_PUBLIC_AUTH0_DOMAIN"
        value = var.auth0_domain
      }
      env {
        name  = "NEXT_PUBLIC_AUTH0_CLIENT_ID"
        value = var.auth0_client_id
      }
      env {
        name  = "NEXT_PUBLIC_AUTH0_AUDIENCE"
        value = var.auth0_audience
      }

      startup_probe {
        transport               = "HTTP"
        port                    = 3000
        path                    = "/api/health"
        initial_delay           = 2
        interval_seconds        = 5
        timeout                 = 3
        failure_count_threshold = 12
      }

      liveness_probe {
        transport               = "HTTP"
        port                    = 3000
        path                    = "/api/health"
        initial_delay           = 5
        interval_seconds        = 15
        timeout                 = 3
        failure_count_threshold = 3
      }

      readiness_probe {
        transport               = "HTTP"
        port                    = 3000
        path                    = "/api/health"
        initial_delay           = 3
        interval_seconds        = 10
        timeout                 = 3
        failure_count_threshold = 3
      }
    }
  }
}

# 3. Loan Database Migrations Container Apps Job (Manual trigger, migrations identity)
resource "azurerm_container_app_job" "migrations" {
  name                         = local.loan_migrations_name
  location                     = var.location
  resource_group_name          = local.resource_group_name
  container_app_environment_id = var.handoff.container_app_environment_id
  workload_profile_name        = "Consumption"
  replica_timeout_in_seconds   = 180
  replica_retry_limit          = 0
  tags                         = merge(local.base_tags, { service = "loan" })

  identity {
    type         = "UserAssigned"
    identity_ids = [var.handoff.identities["migrations"].id]
  }

  registry {
    server   = var.handoff.registry_login_server
    identity = var.handoff.identities["migrations"].id
  }

  manual_trigger_config {
    parallelism              = 1
    replica_completion_count = 1
  }

  template {
    container {
      name    = "loan-migrations"
      image   = var.backend_image
      cpu     = 0.25
      memory  = "0.5Gi"
      command = ["loan-service", "migrate"]

      env {
        name  = "RC_ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "AZURE_CLIENT_ID"
        value = var.handoff.identities["migrations"].client_id
      }
      env {
        name  = "PGHOST"
        value = var.handoff.postgres_host
      }
      env {
        name  = "PGPORT"
        value = "5432"
      }
      env {
        name  = "PGDATABASE"
        value = var.handoff.postgres_database
      }
      env {
        name  = "PGUSER"
        value = var.handoff.identities["migrations"].name
      }
      env {
        name  = "PUBLISHER_PGUSER"
        value = var.handoff.identities["publisher"].name
      }
      env {
        name  = "SEED_DEMO_DATA"
        value = tostring(var.seed_demo_data)
      }
    }
  }
}

# 4. Loan Transactional Outbox Publisher Container Apps Job (Scheduled every minute, publisher identity)
resource "azurerm_container_app_job" "publisher" {
  name                         = local.loan_publisher_name
  location                     = var.location
  resource_group_name          = local.resource_group_name
  container_app_environment_id = var.handoff.container_app_environment_id
  workload_profile_name        = "Consumption"
  replica_timeout_in_seconds   = 60
  replica_retry_limit          = 1
  tags                         = merge(local.base_tags, { service = "loan" })

  identity {
    type         = "UserAssigned"
    identity_ids = [var.handoff.identities["publisher"].id]
  }

  registry {
    server   = var.handoff.registry_login_server
    identity = var.handoff.identities["publisher"].id
  }

  schedule_trigger_config {
    cron_expression          = "*/1 * * * *"
    parallelism              = 1
    replica_completion_count = 1
  }

  template {
    container {
      name    = "loan-publisher"
      image   = var.backend_image
      cpu     = 0.25
      memory  = "0.5Gi"
      command = ["loan-service", "publish"]

      env {
        name  = "RC_ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "AZURE_CLIENT_ID"
        value = var.handoff.identities["publisher"].client_id
      }
      env {
        name  = "PGHOST"
        value = var.handoff.postgres_host
      }
      env {
        name  = "PGPORT"
        value = "5432"
      }
      env {
        name  = "PGDATABASE"
        value = var.handoff.postgres_database
      }
      env {
        name  = "PGUSER"
        value = var.handoff.identities["publisher"].name
      }
      env {
        name  = "SERVICEBUS_NAMESPACE_FQDN"
        value = var.handoff.servicebus_namespace
      }
      env {
        name  = "SERVICEBUS_NAMESPACE"
        value = var.handoff.servicebus_namespace
      }
      env {
        name  = "SERVICEBUS_QUEUE_NAME"
        value = var.handoff.servicebus_queue
      }
      env {
        name  = "SERVICEBUS_QUEUE"
        value = var.handoff.servicebus_queue
      }
      env {
        name  = "OUTBOX_BATCH_SIZE"
        value = "25"
      }
      env {
        name  = "OUTBOX_LEASE_SECONDS"
        value = "60"
      }
    }
  }
}

# 5. Notifications Worker Container App (Independent process, no HTTP ingress, KEDA Service Bus scale-from-zero)
resource "azurerm_container_app" "notifications" {
  name                         = local.notifications_app_name
  container_app_environment_id = var.handoff.container_app_environment_id
  resource_group_name          = local.resource_group_name
  revision_mode                = "Single"
  workload_profile_name        = "Consumption"
  tags                         = merge(local.base_tags, { service = "notifications" })

  identity {
    type         = "UserAssigned"
    identity_ids = [var.handoff.identities["notifications"].id]
  }

  registry {
    server   = var.handoff.registry_login_server
    identity = var.handoff.identities["notifications"].id
  }

  template {
    min_replicas = 0
    max_replicas = 3

    custom_scale_rule {
      name             = "servicebus-queue-rule"
      custom_rule_type = "azure-servicebus"
      identity_id      = var.handoff.identities["notifications"].id
      metadata = {
        namespace              = local.servicebus_namespace
        queueName              = var.handoff.servicebus_queue
        messageCount           = "1"
        activationMessageCount = "0"
      }
    }

    container {
      name    = "notifications-worker"
      image   = var.backend_image
      cpu     = 0.25
      memory  = "0.5Gi"
      command = ["notifications-worker", "worker"]

      env {
        name  = "RC_ENVIRONMENT"
        value = var.environment
      }
      env {
        name  = "AZURE_CLIENT_ID"
        value = var.handoff.identities["notifications"].client_id
      }
      env {
        name  = "SERVICEBUS_NAMESPACE_FQDN"
        value = var.handoff.servicebus_namespace
      }
      env {
        name  = "SERVICEBUS_QUEUE_NAME"
        value = var.handoff.servicebus_queue
      }
      env {
        name  = "COSMOS_ENDPOINT"
        value = var.handoff.cosmos_endpoint
      }
      env {
        name  = "COSMOS_DATABASE"
        value = var.handoff.cosmos_database
      }
      env {
        name  = "COSMOS_CONTAINER"
        value = var.handoff.cosmos_container
      }
      env {
        name  = "KEYVAULT_URI"
        value = var.handoff.notifications_vault_uri
      }
      env {
        name  = "EMAIL_DELIVERY_MODE"
        value = var.email_delivery_mode
      }
      env {
        name  = "PGHOST"
        value = var.handoff.postgres_host
      }
      env {
        name  = "PGDATABASE"
        value = var.handoff.postgres_database
      }
      env {
        name  = "PGUSER"
        value = var.handoff.identities["notifications"].name
      }
    }
  }
}
