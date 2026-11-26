variable "project_id" {
  description = "Projet GCP cible (fourni par TF_VAR_project_id)"
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
  description = "Région GCP"
  type        = string
  default     = "europe-west9"
}

variable "subnet_cidr" {
  description = "Plage primaire du sous-réseau des nœuds"
  type        = string
  default     = "10.30.0.0/20"
}

variable "pods_cidr" {
  description = "Plage secondaire des pods"
  type        = string
  default     = "10.31.0.0/16"
}

variable "services_cidr" {
  description = "Plage secondaire des services"
  type        = string
  default     = "10.32.0.0/20"
}

variable "master_cidr" {
  description = "Plage /28 du plan de contrôle privé"
  type        = string
  default     = "172.16.30.0/28"
}

variable "node_machine_type" {
  description = "Type de machine des nœuds GKE"
  type        = string
  default     = "e2-standard-4"
}

variable "node_min" {
  description = "Nombre minimal de nœuds par zone"
  type        = number
  default     = 1
}

variable "node_max" {
  description = "Nombre maximal de nœuds par zone"
  type        = number
  default     = 3
}
