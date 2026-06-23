# ---------------------------------------------------------------------------
# GCP Professional Cloud DevOps Engineer — Cloud Logging avancé
# Sinks, exclusions, log-based metrics, alerts
# ---------------------------------------------------------------------------

# Log sink : tous les logs GCE vers GCS pour archiving
resource "google_logging_project_sink" "gce_to_gcs" {
  name        = "${var.project_name}-gce-logs-to-gcs-${var.environment}"
  destination = "storage.googleapis.com/${google_storage_bucket.logs.name}"
  filter      = "resource.type=\"gce_instance\""

  unique_writer_identity = true
}

resource "google_storage_bucket" "logs" {
  name          = "${var.project_name}-logs-${var.environment}-${var.gcp_project_id}"
  location      = var.gcp_region
  force_destroy = false

  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  lifecycle_rule {
    action {
      type = "Delete"
    }
    condition {
      age = 90
    }
  }

  labels = local.tags
}

resource "google_storage_bucket_iam_member" "log_writer" {
  bucket = google_storage_bucket.logs.name
  role   = "roles/storage.objectCreator"
  member = google_logging_project_sink.gce_to_gcs.writer_identity
}

# Log sink : audit logs vers BigQuery (analyse)
resource "google_bigquery_dataset" "logs" {
  count       = var.enable_log_analytics ? 1 : 0
  dataset_id  = "${replace(var.project_name, "-", "_")}_logs_${var.environment}"
  description = "Dataset BigQuery pour l'analyse des logs"
  location    = var.gcp_region
  labels      = local.tags
}

resource "google_logging_project_sink" "audit_to_bigquery" {
  count       = var.enable_log_analytics ? 1 : 0
  name        = "${var.project_name}-audit-to-bq-${var.environment}"
  destination = "bigquery.googleapis.com/projects/${var.gcp_project_id}/datasets/${google_bigquery_dataset.logs[0].dataset_id}"
  filter      = "protoPayload.serviceName!=\"\""

  unique_writer_identity = true
}

resource "google_bigquery_dataset_iam_member" "log_writer" {
  count      = var.enable_log_analytics ? 1 : 0
  dataset_id = google_bigquery_dataset.logs[0].dataset_id
  role       = "roles/bigquery.dataEditor"
  member     = google_logging_project_sink.audit_to_bigquery[0].writer_identity
}

# Exclusion : filtrer les logs de health check trop bruyants
resource "google_logging_project_exclusion" "health_check" {
  name        = "${var.project_name}-exclude-health-checks-${var.environment}"
  description = "Exclure les logs de health check frequents"
  filter      = "jsonPayload.message=\"OK\" OR jsonPayload.path=\"/health\""
}

# Log-based metric : compter les erreurs 5xx
resource "google_logging_metric" "errors_5xx" {
  name        = "${var.project_name}-5xx-errors-${var.environment}"
  description = "Nombre d'erreurs HTTP 5xx detectees dans les logs"
  filter      = "resource.type=\"gce_instance\" AND textPayload=~\"5[0-9][0-9]\""

  metric_descriptor {
    metric_kind  = "DELTA"
    value_type   = "INT64"
    unit         = "1"
    description  = "Compteur d'erreurs 5xx"
  }
}

# Log-based metric : latence p95 (a partir de logs JSON structures)
resource "google_logging_metric" "latency_p95" {
  name        = "${var.project_name}-latency-p95-${var.environment}"
  description = "Latence p95 extraite des logs JSON"
  filter      = "resource.type=\"gce_instance\" AND jsonPayload.latency_ms!=\"\""

  metric_descriptor {
    metric_kind  = "DELTA"
    value_type   = "DISTRIBUTION"
    unit         = "ms"
    description  = "Distribution de latence"
  }

  value_extractor = "EXTRACT(jsonPayload.latency_ms)"
  bucket_options {
    exponential_buckets {
      num_finite_buckets = 64
      growth_factor      = 2
      scale              = 1
    }
  }
}

# Alerting policy sur la log-based metric 5xx
resource "google_monitoring_alert_policy" "log_errors" {
  display_name = "${var.project_name}-log-5xx-errors-${var.environment}"
  combiner     = "OR"
  conditions {
    display_name = "5xx errors > 5 in 5 min"
    condition_threshold {
      filter          = "resource.type=\"global\" AND metric.type=\"logging.googleapis.com/user/${google_logging_metric.errors_5xx.name}\""
      duration        = "0s"
      comparison      = "COMPARISON_GT"
      threshold_value = 5
      aggregations {
        alignment_period     = "300s"
        per_series_aligner   = "ALIGN_SUM"
      }
    }
  }

  notification_channels = [google_monitoring_notification_channel.email.id]
  severity              = "ERROR"
}
