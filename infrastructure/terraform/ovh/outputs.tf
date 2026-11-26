output "backup_bucket" {
  description = "Bucket des copies de sauvegarde (dépôt restic secondaire)"
  value       = ovh_cloud_project_storage.backups.name
}

output "kube_cluster_id" {
  description = "Identifiant du cluster Managed Kubernetes (si déployé)"
  value       = one(ovh_cloud_project_kube.main[*].id)
}
