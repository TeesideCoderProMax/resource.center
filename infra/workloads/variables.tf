variable "environment" {
  description = "Explicit deployment environment."
  type        = string
  validation {
    condition     = contains(["test", "staging", "production"], var.environment)
    error_message = "Environment must be test, staging, or production."
  }
}

variable "subscription_id" {
  description = "Azure subscription ID."
  type        = string
}

variable "tenant_id" {
  description = "Microsoft Entra tenant ID."
  type        = string
}

variable "location" {
  description = "Primary Azure region (must match Phase 1 foundation region)."
  type        = string
  default     = "francecentral"
}

variable "name_prefix" {
  description = "Resource naming prefix matching Phase 1 foundation."
  type        = string
  default     = "rc-test-0330980f"
}

variable "tags" {
  description = "Additional tags merged with required Resource Center tags."
  type        = map(string)
  default     = {}
}

variable "handoff" {
  description = "Phase 1 foundation handoff artifact values from docs/cloud/test-foundation-outputs.json (no terraform_remote_state)."
  type = object({
    container_app_environment_id = string
    cosmos_capacity_mode         = string
    cosmos_container             = string
    cosmos_database              = string
    cosmos_endpoint              = string
    environment                  = string
    github_client_id             = string
    identities = map(object({
      client_id    = string
      id           = string
      name         = string
      principal_id = string
    }))
    location                = string
    notifications_vault_uri = string
    platform_resource_group = string
    postgres_database       = string
    postgres_host           = string
    registry_id             = string
    registry_login_server   = string
    registry_sku            = string
    servicebus_namespace    = string
    servicebus_queue        = string
    subscription_id         = string
    tenant_id               = string
    workload_resource_group = string
  })
}

variable "backend_image" {
  description = "Immutable digest-pinned ACR image reference (@sha256:...) for the Rust backend (loan-service & notifications-worker)."
  type        = string
  validation {
    condition     = can(regex("@sha256:[0-9a-f]{64}$", var.backend_image))
    error_message = "backend_image must be pinned to an immutable @sha256:... digest."
  }
}

variable "web_image" {
  description = "Immutable digest-pinned ACR image reference (@sha256:...) for the Next.js web frontend."
  type        = string
  validation {
    condition     = can(regex("@sha256:[0-9a-f]{64}$", var.web_image))
    error_message = "web_image must be pinned to an immutable @sha256:... digest."
  }
}

variable "service_version" {
  description = "Application release/build version string exposed in health endpoints."
  type        = string
  default     = "0.1.0-phase2"
}

variable "seed_demo_data" {
  description = "Whether the migrations job applies idempotent demo device catalogue seed rows."
  type        = bool
  default     = true
}

variable "email_delivery_mode" {
  description = "Notifications worker email mode ('fake' by default so automated tests never send live emails)."
  type        = string
  default     = "fake"
  validation {
    condition     = contains(["fake", "resend"], var.email_delivery_mode)
    error_message = "email_delivery_mode must be 'fake' or 'resend'."
  }
}

variable "auth0_domain" {
  description = "Optional Auth0 tenant domain (empty until external Auth0 tenant is configured)."
  type        = string
  default     = ""
}

variable "auth0_audience" {
  description = "Optional Auth0 API audience identifier (empty until external Auth0 tenant is configured)."
  type        = string
  default     = ""
}

variable "auth0_client_id" {
  description = "Optional Auth0 Regular Web Application client ID (public; its client secret is stored separately)."
  type        = string
  default     = ""
}

variable "alert_notification_email" {
  description = "Operator email address routed by the Azure Monitor Action Group in addition to the Subscription Owner role receiver."
  type        = string
  default     = "G6605870@live.tees.ac.uk"
}
