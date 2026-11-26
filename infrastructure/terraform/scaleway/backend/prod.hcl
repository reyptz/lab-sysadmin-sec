# État Terraform scaleway (prod) : Object Storage S3 compatible du fournisseur.
bucket                      = "lab-sysadmin-sec-tfstate"
key                         = "scaleway/prod/terraform.tfstate"
region                      = "fr-par"
endpoints                   = { s3 = "https://s3.fr-par.scw.cloud" }
use_path_style              = true
use_lockfile                = true
skip_credentials_validation = true
skip_region_validation      = true
skip_requesting_account_id  = true
skip_s3_checksum            = true
