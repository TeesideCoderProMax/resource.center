variable "subscription_id" { type = string }
variable "tenant_id" { type = string }
variable "location" { type = string }
variable "environment" {
  type = string
  validation {
    condition     = contains(["test", "staging", "production"], var.environment)
    error_message = "Choose test, staging or production explicitly."
  }
}
variable "name_suffix" {
  type = string
  validation {
    condition     = can(regex("^[a-z0-9]{4,8}$", var.name_suffix))
    error_message = "Use 4–8 lowercase letters or digits for globally unique names."
  }
}
locals {
  prefix  = "rc-${var.environment}-${var.name_suffix}"
  compact = "rc${var.environment}${var.name_suffix}"
  tags    = { project = "resource-center", environment = var.environment, managed_by = "terraform" }
}

variable "cosmos_capacity_mode" {
  description = "Free tier requires the subscription's one available slot. No automatic paid fallback. Changing an applied account requires migration review."
  type        = string
  default     = "free_tier"
  validation {
    condition     = contains(["free_tier", "serverless"], var.cosmos_capacity_mode)
    error_message = "Choose free_tier or explicitly opt into metered serverless."
  }
}
variable "registry_sku" {
  description = "Standard matches the eligible 12-month student offer; use Basic only after checking benefit expiry and retail costs."
  type        = string
  default     = "Standard"
  validation {
    condition     = contains(["Basic", "Standard"], var.registry_sku)
    error_message = "Use Basic or Standard."
  }
}

variable "postgres_zone" {
  description = "Record Azure's selected zone after first creation so subsequent plans preserve it."
  type        = string
  default     = null
}
