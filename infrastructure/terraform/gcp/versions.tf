terraform {
  required_version = ">= 1.10.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }

  # État distant GCS (verrouillage natif), un préfixe par environnement :
  # backend/<env>.hcl.
  backend "gcs" {}
}

provider "google" {
  project = var.project_id
  region  = var.region
}
