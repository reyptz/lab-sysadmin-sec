# ---------------------------------------------------------------------------
# GCP Professional Cloud DevOps Engineer — GKE, Cloud Run, Cloud Functions
# ---------------------------------------------------------------------------

# GKE Autopilot cluster
resource "google_container_cluster" "main" {
  name     = "${var.project_name}-gke-${var.environment}"
  location = var.gcp_region
  network  = google_compute_network.vpc.id

  subnetwork = google_compute_subnetwork.private[0].id

  enable_autopilot = true

  private_cluster_config {
    enable_private_nodes    = true
    enable_private_endpoint = false
    master_ipv4_cidr_block  = "172.16.0.0/28"
  }

  ip_allocation_policy {
    cluster_secondary_range_name  = "pods"
    services_secondary_range_name = "services"
  }

  release_channel {
    channel = "REGULAR"
  }

  workload_identity_config {
    workload_pool = "${var.gcp_project_id}.svc.id.goog"
  }

  depends_on = [google_compute_subnetwork_secondary_range.pods, google_compute_subnetwork_secondary_range.services]

  labels = local.tags
}

# Secondary IP ranges for GKE pods and services
resource "google_compute_subnetwork_secondary_range" "pods" {
  name          = "pods"
  subnetwork    = google_compute_subnetwork.private[0].id
  region        = var.gcp_region
  ip_cidr_range = "10.3.0.0/17"
}

resource "google_compute_subnetwork_secondary_range" "services" {
  name          = "services"
  subnetwork    = google_compute_subnetwork.private[0].id
  region        = var.gcp_region
  ip_cidr_range = "10.3.128.0/20"
}

# Cloud Run service
resource "google_cloud_run_v2_service" "app" {
  count    = var.enable_cloud_run ? 1 : 0
  name     = "${var.project_name}-app-${var.environment}"
  location = var.gcp_region
  ingress  = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"

  template {
    containers {
      image = "${var.gcp_region}-docker.pkg.dev/${var.gcp_project_id}/${google_artifact_registry_repository.app.name}/app:latest"
      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
      }
      ports {
        container_port = 8080
      }
    }
    service_account = google_service_account.cloud_run[0].email
  }

  labels = local.tags
}

resource "google_service_account" "cloud_run" {
  count        = var.enable_cloud_run ? 1 : 0
  account_id   = "${var.project_name}-cloudrun-${var.environment}"
  display_name = "Cloud Run Service Account"
}

resource "google_cloud_run_v2_service_iam_member" "invoker" {
  count    = var.enable_cloud_run ? 1 : 0
  name     = google_cloud_run_v2_service.app[0].name
  location = var.gcp_region
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.cloud_run[0].email}"
}

resource "google_cloud_run_v2_service_iam_member" "authenticated" {
  count    = var.enable_cloud_run ? 1 : 0
  name     = google_cloud_run_v2_service.app[0].name
  location = var.gcp_region
  role     = "roles/run.invoker"
  member   = "allAuthenticatedUsers"
}

# Cloud Function (HTTP)
resource "google_cloudfunctions_function" "health_check" {
  count                 = var.enable_cloud_functions ? 1 : 0
  name                  = "${var.project_name}-health-check-${var.environment}"
  description           = "Health check function for the lab"
  runtime               = "python312"
  available_memory_mb   = 256
  source_archive_bucket = google_storage_bucket.functions_source[0].name
  source_archive_object = google_storage_bucket_object.health_check[0].name
  trigger_http          = true
  entry_point           = "handler"
  service_account_email = google_service_account.cloud_functions[0].email

  labels = local.tags
}

resource "google_service_account" "cloud_functions" {
  count        = var.enable_cloud_functions ? 1 : 0
  account_id   = "${var.project_name}-cloudfn-${var.environment}"
  display_name = "Cloud Functions Service Account"
}

resource "google_storage_bucket" "functions_source" {
  count                       = var.enable_cloud_functions ? 1 : 0
  name                        = "${var.gcp_project_id}-functions-source"
  location                    = var.gcp_region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning { enabled = true }

  labels = local.tags
}

resource "google_storage_bucket_object" "health_check" {
  count  = var.enable_cloud_functions ? 1 : 0
  name   = "health-check-function.zip"
  bucket = google_storage_bucket.functions_source[0].name
  source = data.archive_file.health_check[0].output_path
}

# Local archive for Cloud Function
 data "archive_file" "health_check" {
  count       = var.enable_cloud_functions ? 1 : 0
  type        = "zip"
  output_path = "${path.module}/health_check_function.zip"
  source {
    content  = <<EOF
import json

def handler(request):
    return (json.dumps({"status": "ok"}), 200, {"Content-Type": "application/json"})
EOF
    filename = "main.py"
  }
}
