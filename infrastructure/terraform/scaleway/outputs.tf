output "backup_bucket" {
  description = "Bucket des copies de sauvegarde (dépôt restic secondaire)"
  value       = scaleway_object_bucket.backups.name
}

output "kapsule_cluster_id" {
  description = "Identifiant du cluster Kapsule (si déployé)"
  value       = one(scaleway_k8s_cluster.main[*].id)
}
