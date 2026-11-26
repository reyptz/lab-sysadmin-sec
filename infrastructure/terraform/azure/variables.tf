variable "subscription_id" {
  description = "Abonnement Azure cible (fourni par TF_VAR_subscription_id)"
  type        = string
}

variable "project_name" {
  description = "Nom du projet (préfixe des ressources)"
  type        = string
  default     = "lab-sysadmin-sec"
}

variable "environment" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "location" {
  description = "Région Azure"
  type        = string
  default     = "francecentral"
}

variable "vnet_cidr" {
  description = "Plage d'adresses du réseau virtuel"
  type        = string
  default     = "10.20.0.0/16"
}

variable "aks_node_count" {
  description = "Nombre de nœuds du pool système AKS"
  type        = number
  default     = 2
}

variable "aks_node_size" {
  description = "Taille des nœuds AKS"
  type        = string
  default     = "Standard_D4s_v5"
}

variable "rhel_vm_count" {
  description = "Nombre de VMs RHEL (cible Ansible de reprise)"
  type        = number
  default     = 0
}

variable "rhel_vm_size" {
  description = "Taille des VMs RHEL"
  type        = string
  default     = "Standard_B2s"
}

variable "admin_username" {
  description = "Utilisateur d'administration des VMs RHEL"
  type        = string
  default     = "labadmin"
}

variable "admin_ssh_public_key" {
  description = "Clé publique SSH d'administration (TF_VAR_admin_ssh_public_key)"
  type        = string
  default     = ""
}
