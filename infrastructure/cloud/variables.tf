variable "aws_region" {
  description = "Région AWS de déploiement"
  type        = string
  default     = "eu-west-3"
}

variable "environment" {
  description = "Nom de l'environnement (dev, staging, prod)"
  type        = string
  default     = "dev"
}

variable "project_name" {
  description = "Nom du projet pour le tagging"
  type        = string
  default     = "lab-sysadmin-sec"
}

variable "vpc_cidr" {
  description = "CIDR du VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "Zones de disponibilité à utiliser"
  type        = list(string)
  default     = ["eu-west-3a", "eu-west-3b"]
}

variable "public_subnet_cidrs" {
  description = "CIDRs des subnets publics"
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24"]
}

variable "private_subnet_cidrs" {
  description = "CIDRs des subnets privés"
  type        = list(string)
  default     = ["10.0.10.0/24", "10.0.20.0/24"]
}

variable "instance_type_bastion" {
  description = "Type d'instance pour le bastion"
  type        = string
  default     = "t3.micro"
}

variable "instance_type_app" {
  description = "Type d'instance pour les serveurs applicatifs"
  type        = string
  default     = "t3.small"
}

variable "allowed_ssh_cidr" {
  description = "CIDR autorisé à se connecter en SSH au bastion"
  type        = string
  default     = "0.0.0.0/0"
  sensitive   = true
}

variable "key_pair_name" {
  description = "Nom de la paire de clés SSH existante (laisser vide pour utiliser SSM)"
  type        = string
  default     = ""
}

variable "organization_id" {
  description = "ID de l'AWS Organization (vide pour ignorer les ressources Organizations)"
  type        = string
  default     = ""
}

variable "organization_root_id" {
  description = "Root ID de l'AWS Organization"
  type        = string
  default     = ""
}

variable "db_password" {
  description = "Mot de passe admin pour RDS/Aurora"
  type        = string
  sensitive   = true
  default     = "ChangeMe123!"
}

variable "domain_name" {
  description = "Nom de domaine Route53/CloudFront (vide pour ignorer)"
  type        = string
  default     = ""
}

variable "enable_dms" {
  description = "Activer AWS Database Migration Service"
  type        = bool
  default     = false
}

variable "enable_config" {
  description = "Activer AWS Config"
  type        = bool
  default     = false
}

variable "enable_guardduty" {
  description = "Activer Amazon GuardDuty"
  type        = bool
  default     = false
}

variable "enable_securityhub" {
  description = "Activer AWS Security Hub"
  type        = bool
  default     = false
}

variable "enable_waf" {
  description = "Activer AWS WAF"
  type        = bool
  default     = false
}

variable "enable_kinesis" {
  description = "Activer Kinesis Data Streams / Firehose"
  type        = bool
  default     = false
}

variable "monthly_budget" {
  description = "Budget mensuel en USD"
  type        = string
  default     = "100"
}

variable "budget_alert_emails" {
  description = "Liste d'emails pour les alertes de budget"
  type        = list(string)
  default     = []
}

variable "alert_emails" {
  description = "Liste d'emails pour les alertes SNS"
  type        = list(string)
  default     = []
}
