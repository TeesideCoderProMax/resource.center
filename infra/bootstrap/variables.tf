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
