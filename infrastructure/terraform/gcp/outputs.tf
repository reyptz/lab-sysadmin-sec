output "gke_cluster_name" {
  description = "Cluster GKE (contexte kubeconfig : gcloud container clusters get-credentials)"
  value       = google_container_cluster.main.name
}

output "gke_endpoint" {
  description = "Point d'accès du plan de contrôle"
  value       = google_container_cluster.main.endpoint
  sensitive   = true
}

output "workload_identity_pool" {
  description = "Pool Workload Identity (fédération des comptes de service Kubernetes)"
  value       = "${var.project_id}.svc.id.goog"
}
