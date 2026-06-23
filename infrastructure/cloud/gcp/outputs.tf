output "vpc_id" {
  description = "ID du VPC GCP"
  value       = google_compute_network.vpc.id
}

output "public_subnet_id" {
  description = "ID du subnet public"
  value       = google_compute_subnetwork.public.id
}

output "private_subnet_ids" {
  description = "IDs des subnets prives"
  value       = google_compute_subnetwork.private[*].id
}

output "bastion_public_ip" {
  description = "IP publique du bastion"
  value       = google_compute_instance.bastion.network_interface[0].access_config[0].nat_ip
}

output "app_private_ips" {
  description = "IPs privees des serveurs applicatifs"
  value       = google_compute_instance.app[*].network_interface[0].network_ip
}

output "backup_bucket_name" {
  description = "Nom du bucket GCS de backup"
  value       = google_storage_bucket.backups.name
}

output "monitoring_dashboard" {
  description = "Nom du dashboard Cloud Monitoring"
  value       = google_monitoring_dashboard.lab_dashboard.display_name
}

output "artifact_registry_repository" {
  description = "Repository Artifact Registry"
  value       = google_artifact_registry_repository.app.name
}

output "cloudbuild_trigger" {
  description = "Nom du Cloud Build trigger GitHub (si configure)"
  value       = length(google_cloudbuild_trigger.app) > 0 ? google_cloudbuild_trigger.app[0].name : ""
}

output "gke_cluster_name" {
  description = "Nom du cluster GKE"
  value       = google_container_cluster.main.name
}

output "cloud_run_service" {
  description = "Nom du service Cloud Run (si active)"
  value       = length(google_cloud_run_v2_service.app) > 0 ? google_cloud_run_v2_service.app[0].name : ""
}

output "cloud_function_name" {
  description = "Nom de la Cloud Function (si active)"
  value       = length(google_cloudfunctions_function.health_check) > 0 ? google_cloudfunctions_function.health_check[0].name : ""
}

output "log_bucket_name" {
  description = "Bucket GCS pour les logs archives"
  value       = google_storage_bucket.logs.name
}

output "log_sink_name" {
  description = "Nom du sink Cloud Logging vers GCS"
  value       = google_logging_project_sink.gce_to_gcs.name
}

output "log_based_metric_5xx" {
  description = "Nom de la metrique log-based 5xx"
  value       = google_logging_metric.errors_5xx.name
}
