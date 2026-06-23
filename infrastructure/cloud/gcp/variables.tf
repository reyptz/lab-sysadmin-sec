variable "gcp_project_id" {
  description = "ID du projet GCP"
  type        = string
}

variable "gcp_region" {
  description = "Region GCP de deploiement"
  type        = string
  default     = "europe-west1"
}

variable "gcp_zone" {
  description = "Zone principale"
  type        = string
  default     = "europe-west1-b"
}

variable "environment" {
  description = "Nom de l'environnement"
  type        = string
  default     = "dev"
}

variable "project_name" {
  description = "Nom du projet"
  type        = string
  default     = "lab-sysadmin-sec"
}

variable "vpc_cidr" {
  description = "CIDR du VPC"
  type        = string
  default     = "10.2.0.0/16"
}

variable "public_subnet_cidr" {
  description = "CIDR du subnet public"
  type        = string
  default     = "10.2.1.0/24"
}

variable "private_subnet_cidrs" {
  description = "CIDRs des subnets prives"
  type        = list(string)
  default     = ["10.2.10.0/24", "10.2.20.0/24"]
}

variable "machine_type_bastion" {
  description = "Type de machine pour le bastion"
  type        = string
  default     = "e2-micro"
}

variable "machine_type_app" {
  description = "Type de machine pour les serveurs applicatifs"
  type        = string
  default     = "e2-small"
}

variable "allowed_ssh_cidr" {
  description = "CIDR autorise en SSH sur le bastion"
  type        = string
  default     = "0.0.0.0/0"
  sensitive   = true
}

variable "github_owner" {
  description = "Proprietaire GitHub pour le Cloud Build trigger"
  type        = string
  default     = ""
}

variable "github_repo" {
  description = "Nom du repo GitHub pour le Cloud Build trigger"
  type        = string
  default     = ""
}

variable "enable_cloud_deploy" {
  description = "Activer Google Cloud Deploy"
  type        = bool
  default     = false
}

variable "enable_cloud_run" {
  description = "Activer Cloud Run"
  type        = bool
  default     = false
}

variable "enable_cloud_functions" {
  description = "Activer Cloud Functions"
  type        = bool
  default     = false
}

variable "enable_log_analytics" {
  description = "Activer BigQuery pour l'analyse des logs"
  type        = bool
  default     = false
}

variable "alert_email" {
  description = "Email pour les alertes GCP"
  type        = string
  default     = "ops@example.com"
}
