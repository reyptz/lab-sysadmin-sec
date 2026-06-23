variable "azure_region" {
  description = "Region Azure de deploiement"
  type        = string
  default     = "West Europe"
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

variable "vnet_cidr" {
  description = "CIDR du VNet"
  type        = string
  default     = "10.1.0.0/16"
}

variable "public_subnet_cidrs" {
  description = "CIDRs des subnets publics"
  type        = list(string)
  default     = ["10.1.1.0/24", "10.1.2.0/24"]
}

variable "private_subnet_cidrs" {
  description = "CIDRs des subnets prives"
  type        = list(string)
  default     = ["10.1.10.0/24", "10.1.20.0/24"]
}

variable "vm_size_bastion" {
  description = "Taille de la VM bastion"
  type        = string
  default     = "Standard_B1s"
}

variable "vm_size_app" {
  description = "Taille des VMs applicatives"
  type        = string
  default     = "Standard_B2s"
}

variable "admin_username" {
  description = "Nom d'administrateur local des VMs"
  type        = string
  default     = "labadmin"
}

variable "allowed_ssh_cidr" {
  description = "CIDR autorise en SSH sur le bastion"
  type        = string
  default     = "0.0.0.0/0"
  sensitive   = true
}

variable "azure_secondary_region" {
  description = "Region Azure secondaire pour la geo-replication et le Site Recovery"
  type        = string
  default     = "North Europe"
}

variable "sql_admin_password" {
  description = "Mot de passe admin pour Azure SQL Server"
  type        = string
  sensitive   = true
  default     = "ChangeMe123!"
}

variable "enable_conditional_access" {
  description = "Activer la Conditional Access MFA pour les admins Entra ID"
  type        = bool
  default     = false
}

variable "enable_application_gateway" {
  description = "Activer Azure Application Gateway"
  type        = bool
  default     = false
}

variable "enable_front_door" {
  description = "Activer Azure Front Door"
  type        = bool
  default     = false
}

variable "enable_site_recovery" {
  description = "Activer Azure Site Recovery (couteux)"
  type        = bool
  default     = false
}

variable "enable_defender" {
  description = "Activer Microsoft Defender for Cloud"
  type        = bool
  default     = false
}

variable "enable_sentinel" {
  description = "Activer Microsoft Sentinel"
  type        = bool
  default     = false
}

variable "enable_vpn_gateway" {
  description = "Activer Azure VPN Gateway"
  type        = bool
  default     = false
}

variable "enable_expressroute" {
  description = "Activer Azure ExpressRoute Gateway"
  type        = bool
  default     = false
}

variable "enable_private_link" {
  description = "Activer Azure Private Link pour SQL Server"
  type        = bool
  default     = false
}

variable "alert_emails" {
  description = "Liste d'emails pour les alertes de securite"
  type        = list(string)
  default     = []
}

variable "onprem_gateway_ip" {
  description = "Adresse IP publique du gateway on-premises"
  type        = string
  default     = "203.0.113.1"
}

variable "onprem_address_spaces" {
  description = "Espaces d'adresses du reseau on-premises"
  type        = list(string)
  default     = ["192.168.19.0/24", "192.168.23.0/24", "192.168.100.0/24"]
}

variable "vpn_shared_key" {
  description = "Cle partagee pour la connexion VPN Site-to-Site"
  type        = string
  sensitive   = true
  default     = "ChangeThisSharedKey!"
}
