# Zone d'accueil Scaleway : seconde copie souveraine des sauvegardes (règle
# 3-2-1) et cluster Kapsule optionnel sur réseau privé.

locals {
  name = "${var.project_name}-${var.environment}"
  tags = ["project:${var.project_name}", "environment:${var.environment}", "managed_by:socle"]
}

resource "scaleway_object_bucket" "backups" {
  name   = "${local.name}-dr"
  region = var.region

  tags = {
    project     = var.project_name
    environment = var.environment
  }

  versioning {
    enabled = true
  }
}

resource "scaleway_vpc_private_network" "main" {
  count  = var.deploy_kubernetes ? 1 : 0
  name   = "pn-${local.name}"
  region = var.region
  tags   = local.tags
}

resource "scaleway_k8s_cluster" "main" {
  count                       = var.deploy_kubernetes ? 1 : 0
  name                        = "kapsule-${local.name}"
  version                     = var.kube_version
  cni                         = "cilium"
  private_network_id          = scaleway_vpc_private_network.main[0].id
  delete_additional_resources = false
  tags                        = local.tags

  auto_upgrade {
    enable                        = true
    maintenance_window_start_hour = 3
    maintenance_window_day        = "sunday"
  }
}

resource "scaleway_k8s_pool" "main" {
  count       = var.deploy_kubernetes ? 1 : 0
  cluster_id  = scaleway_k8s_cluster.main[0].id
  name        = "principal"
  node_type   = var.node_type
  size        = var.node_min
  min_size    = var.node_min
  max_size    = var.node_max
  autoscaling = true
  autohealing = true
  tags        = local.tags
}
