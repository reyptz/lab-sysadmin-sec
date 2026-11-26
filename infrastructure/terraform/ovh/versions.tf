terraform {
  required_version = ">= 1.10.0"

  required_providers {
    ovh = {
      source  = "ovh/ovh"
      version = "~> 1.0"
    }
  }

  # État distant sur l'Object Storage S3 d'OVHcloud (backend s3 compatible),
  # un fichier par environnement : backend/<env>.hcl.
  backend "s3" {}
}

# Identifiants : OVH_APPLICATION_KEY, OVH_APPLICATION_SECRET, OVH_CONSUMER_KEY
# (ou OAuth2 OVH_CLIENT_ID / OVH_CLIENT_SECRET), jamais dans le dépôt.
provider "ovh" {
  endpoint = "ovh-eu"
}
