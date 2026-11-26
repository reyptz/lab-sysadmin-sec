terraform {
  required_version = ">= 1.10.0"

  required_providers {
    scaleway = {
      source  = "scaleway/scaleway"
      version = "~> 2.0"
    }
  }

  # État distant sur l'Object Storage S3 de Scaleway (backend s3 compatible),
  # un fichier par environnement : backend/<env>.hcl.
  backend "s3" {}
}

# Identifiants : SCW_ACCESS_KEY, SCW_SECRET_KEY, SCW_DEFAULT_PROJECT_ID.
provider "scaleway" {
  region = var.region
  zone   = "${var.region}-1"
}
