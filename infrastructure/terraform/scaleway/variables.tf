variable "project_name" {
  description = "Nom du projet (préfixe des ressources)"
  type        = string
  default     = "lab-sysadmin-sec"
}

variable "environment" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "region" {
  description = "Région Scaleway"
  type        = string
  default     = "fr-par"
}

variable "kube_version" {
  description = "Version mineure de Kubernetes"
  type        = string
  default     = "1.31"
}

variable "node_type" {
  description = "Type d'instance des nœuds"
  type        = string
  default     = "PRO2-S"
}

variable "node_min" {
  description = "Nombre minimal de nœuds"
  type        = number
  default     = 1
}

variable "node_max" {
  description = "Nombre maximal de nœuds"
  type        = number
  default     = 3
}

variable "deploy_kubernetes" {
  description = "Créer le cluster Kapsule (sinon, stockage seul)"
  type        = bool
  default     = false
}
