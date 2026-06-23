# ---------------------------------------------------------------------------
# GCP Professional Cloud DevOps Engineer — Cloud Build, Cloud Deploy, Artifact Registry
# ---------------------------------------------------------------------------

# Artifact Registry repository for Docker images
resource "google_artifact_registry_repository" "app" {
  location      = var.gcp_region
  repository_id = "${var.project_name}-app-${var.environment}"
  description   = "Registry Docker pour l'application du lab"
  format        = "DOCKER"
  labels        = local.tags

  cleanup_policies {
    id     = "keep-minimum-versions"
    action = "KEEP"
    most_recent_versions {
      keep_count = 5
    }
  }
}

# Service account for Cloud Build
resource "google_service_account" "cloudbuild" {
  account_id   = "${var.project_name}-cloudbuild-${var.environment}"
  display_name = "Cloud Build Service Account"
}

resource "google_project_iam_member" "cloudbuild_run" {
  project = var.gcp_project_id
  role    = "roles/run.admin"
  member  = "serviceAccount:${google_service_account.cloudbuild.email}"
}

resource "google_project_iam_member" "cloudbuild_artifact_registry" {
  project = var.gcp_project_id
  role    = "roles/artifactregistry.writer"
  member  = "serviceAccount:${google_service_account.cloudbuild.email}"
}

# Cloud Build trigger (GitHub connection)
resource "google_cloudbuild_trigger" "app" {
  count       = var.github_repo != "" ? 1 : 0
  location    = var.gcp_region
  name        = "${var.project_name}-app-trigger-${var.environment}"
  description = "Build and push Docker image on push to main"

  github {
    owner = var.github_owner
    name  = var.github_repo
    push {
      branch = "^main$"
    }
  }

  filename = "cloudbuild.yaml"

  substitutions = {
    _REGION     = var.gcp_region
    _REPOSITORY = google_artifact_registry_repository.app.name
    _PROJECT    = var.gcp_project_id
  }

  service_account = google_service_account.cloudbuild.id
}

# Cloud Deploy pipeline
resource "google_clouddeploy_delivery_pipeline" "app" {
  count    = var.enable_cloud_deploy ? 1 : 0
  location = var.gcp_region
  name     = "${var.project_name}-pipeline-${var.environment}"

  description = "Pipeline de deploiement Canary pour l'application"

  serial_pipeline {
    stages {
      target_id = google_clouddeploy_target.dev[0].target_id
      profiles  = ["dev"]
    }
    stages {
      target_id = google_clouddeploy_target.prod[0].target_id
      profiles  = ["prod"]
      strategy {
        canary {
          runtime_config {
            kubernetes {
              service_networking {
                service = "app"
                deployment = "app"
              }
            }
          }
          canary_deployment {
            percentages = [25, 50, 75]
            verify      = true
          }
        }
      }
    }
  }

  labels = local.tags
}

resource "google_clouddeploy_target" "dev" {
  count    = var.enable_cloud_deploy ? 1 : 0
  location = var.gcp_region
  name     = "${var.project_name}-dev-${var.environment}"
  gke {
    cluster = google_container_cluster.main.id
  }
  require_approval = false
  labels             = local.tags
}

resource "google_clouddeploy_target" "prod" {
  count    = var.enable_cloud_deploy ? 1 : 0
  location = var.gcp_region
  name     = "${var.project_name}-prod-${var.environment}"
  gke {
    cluster = google_container_cluster.main.id
  }
  require_approval = true
  labels             = local.tags
}
