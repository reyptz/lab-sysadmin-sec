# Zone d'accueil OVHcloud : copies de sauvegarde souveraines (règle 3-2-1,
# dépôts restic `copies` de socle.yaml) et cluster Managed Kubernetes optionnel.

locals {
  name = "${var.project_name}-${var.environment}"
}

# Bucket des copies de sauvegarde : versionné et chiffré. Activer en plus le
# verrouillage d'objets (WORM) à la création pour résister aux rançongiciels.
resource "ovh_cloud_project_storage" "backups" {
  service_name = var.service_name
  region_name  = var.storage_region
  name         = "${local.name}-dr"

  versioning = {
    status = "enabled"
  }

  encryption = {
    sse_algorithm = "AES256"
  }
}

resource "ovh_cloud_project_kube" "main" {
  count        = var.deploy_kubernetes ? 1 : 0
  service_name = var.service_name
  name         = "mks-${local.name}"
  region       = var.region
  version      = var.kube_version
}

resource "ovh_cloud_project_kube_nodepool" "main" {
  count         = var.deploy_kubernetes ? 1 : 0
  service_name  = var.service_name
  kube_id       = ovh_cloud_project_kube.main[0].id
  name          = "principal"
  flavor_name   = var.node_flavor
  desired_nodes = var.node_min
  min_nodes     = var.node_min
  max_nodes     = var.node_max
  autoscale     = true
}
