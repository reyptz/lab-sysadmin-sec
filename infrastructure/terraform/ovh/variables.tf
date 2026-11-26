variable "service_name" {
  description = "Identifiant du projet Public Cloud OVHcloud (TF_VAR_service_name)"
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

variable "region" {
  description = "Région OVHcloud"
  type        = string
  default     = "GRA11"
}

variable "storage_region" {
  description = "Région de l'Object Storage (copies de sauvegarde)"
  type        = string
  default     = "GRA"
}

variable "kube_version" {
  description = "Version mineure de Kubernetes"
  type        = string
  default     = "1.31"
}

variable "node_flavor" {
  description = "Gabarit des nœuds"
  type        = string
  default     = "b3-8"
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
  description = "Créer le cluster Managed Kubernetes (sinon, stockage seul)"
  type        = bool
  default     = false
}
