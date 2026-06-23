locals {
  tags = {
    project     = var.project_name
    environment = var.environment
    managed_by  = "terraform"
  }
}

# ---------------------------------------------------------------------------
# VPC & Subnets (GCP Cloud DevOps : reseau cloud-native, HA)
# ---------------------------------------------------------------------------
resource "google_compute_network" "vpc" {
  name                    = "${var.project_name}-vpc-${var.environment}"
  auto_create_subnetworks = false
  routing_mode            = "GLOBAL"
}

resource "google_compute_subnetwork" "public" {
  name          = "${var.project_name}-public-subnet"
  ip_cidr_range = var.public_subnet_cidr
  region        = var.gcp_region
  network       = google_compute_network.vpc.id
}

resource "google_compute_subnetwork" "private" {
  count         = length(var.private_subnet_cidrs)
  name          = "${var.project_name}-private-subnet-${count.index + 1}"
  ip_cidr_range = var.private_subnet_cidrs[count.index]
  region        = var.gcp_region
  network       = google_compute_network.vpc.id
  private_ip_google_access = true
}

# ---------------------------------------------------------------------------
# Firewall (equivalent Security Groups / NSG)
# ---------------------------------------------------------------------------
resource "google_compute_firewall" "allow_ssh_bastion" {
  name        = "${var.project_name}-allow-ssh-bastion"
  network     = google_compute_network.vpc.id
  description = "Autorise SSH sur le bastion depuis le reseau admin"
  direction   = "INGRESS"

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = [var.allowed_ssh_cidr]
  target_tags   = ["bastion"]
}

resource "google_compute_firewall" "allow_app_dmz" {
  name        = "${var.project_name}-allow-app-dmz"
  network     = google_compute_network.vpc.id
  description = "Autorise HTTP/HTTPS sur les serveurs applicatifs"
  direction   = "INGRESS"

  allow {
    protocol = "tcp"
    ports    = ["80", "443"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["app"]
}

resource "google_compute_firewall" "allow_internal" {
  name        = "${var.project_name}-allow-internal"
  network     = google_compute_network.vpc.id
  description = "Trafic interne au VPC"
  direction   = "INGRESS"

  allow {
    protocol = "all"
  }

  source_ranges = [var.vpc_cidr]
}

# ---------------------------------------------------------------------------
# IAP / Bastion (Cloud Identity-Aware Proxy possible, ici bastion classique)
# ---------------------------------------------------------------------------
resource "google_compute_address" "bastion" {
  name   = "${var.project_name}-bastion-ip-${var.environment}"
  region = var.gcp_region
}

resource "google_compute_instance" "bastion" {
  name         = "${var.project_name}-bastion-${var.environment}"
  machine_type = var.machine_type_bastion
  zone         = var.gcp_zone
  tags         = ["bastion"]
  labels       = local.tags

  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2404-lts-amd64"
      size  = 20
      type  = "pd-balanced"
    }
    kms_key_self_link = null
  }

  network_interface {
    subnetwork = google_compute_subnetwork.public.id
    access_config {
      nat_ip = google_compute_address.bastion.address
    }
  }

  metadata = {
    enable-oslogin = "TRUE"
    user-data      = file("${path.module}/cloud-init.yml")
  }

  service_account {
    email  = google_service_account.vm_sa.email
    scopes = ["cloud-platform"]
  }

  shielded_instance_config {
    enable_secure_boot          = true
    enable_vtpm                 = true
    enable_integrity_monitoring = true
  }
}

resource "google_compute_instance" "app" {
  count        = 2
  name         = "${var.project_name}-app-${count.index + 1}-${var.environment}"
  machine_type = var.machine_type_app
  zone         = var.gcp_zone
  tags         = ["app"]
  labels       = local.tags

  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2404-lts-amd64"
      size  = 30
      type  = "pd-balanced"
    }
  }

  network_interface {
    subnetwork = google_compute_subnetwork.private[count.index].id
    # Pas d'IP publique : acces via bastion ou IAP
  }

  metadata = {
    enable-oslogin = "TRUE"
    user-data      = file("${path.module}/cloud-init.yml")
  }

  service_account {
    email  = google_service_account.vm_sa.email
    scopes = ["cloud-platform"]
  }
}

# ---------------------------------------------------------------------------
# IAM Service Account (moindre privilege)
# ---------------------------------------------------------------------------
resource "google_service_account" "vm_sa" {
  account_id   = "${var.project_name}-vm-sa-${var.environment}"
  display_name = "Service Account des VMs du lab"
}

resource "google_project_iam_member" "monitoring" {
  project = var.gcp_project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.vm_sa.email}"
}

resource "google_project_iam_member" "logging" {
  project = var.gcp_project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.vm_sa.email}"
}

# ---------------------------------------------------------------------------
# Observabilite : Cloud Monitoring + Cloud Logging
# ---------------------------------------------------------------------------
resource "google_monitoring_dashboard" "lab_dashboard" {
  dashboard_json = jsonencode({
    displayName = "Lab SysAdmin Sec"
    gridLayout = {
      columns = "2"
      widgets = [
        {
          title = "CPU Usage"
          xyChart = {
            dataSets = [{
              timeSeriesQuery = {
                timeSeriesFilter = {
                  filter = "resource.type=\"gce_instance\" metric.type=\"compute.googleapis.com/instance/cpu/utilization\""
                }
              }
            }]
          }
        }
      ]
    }
  })
}

resource "google_monitoring_alert_policy" "high_cpu" {
  display_name = "${var.project_name}-high-cpu-${var.environment}"
  combiner     = "OR"
  conditions {
    display_name = "CPU > 80%"
    condition_threshold {
      filter          = "resource.type=\"gce_instance\" AND metric.type=\"compute.googleapis.com/instance/cpu/utilization\""
      duration        = "300s"
      comparison      = "COMPARISON_GT"
      threshold_value = 0.8
      aggregations {
        alignment_period     = "300s"
        per_series_aligner   = "ALIGN_MEAN"
      }
    }
  }

  notification_channels = [google_monitoring_notification_channel.email.id]
  severity              = "WARNING"
}

resource "google_monitoring_notification_channel" "email" {
  display_name = "${var.project_name}-alerts-email"
  type         = "email"
  labels = {
    email_address = var.alert_email
  }
}

# ---------------------------------------------------------------------------
# Stockage GCS (backups, logs, chiffre par defaut)
# ---------------------------------------------------------------------------
resource "google_storage_bucket" "backups" {
  name          = "${var.project_name}-backups-${var.environment}-${var.gcp_project_id}"
  location      = var.gcp_region
  force_destroy = false

  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning {
    enabled = true
  }

  labels = local.tags
}
