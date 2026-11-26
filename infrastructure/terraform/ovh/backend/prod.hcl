# État Terraform ovh (prod) : Object Storage S3 compatible du fournisseur.
bucket                      = "lab-sysadmin-sec-tfstate"
key                         = "ovh/prod/terraform.tfstate"
region                      = "gra"
endpoints                   = { s3 = "https://s3.gra.io.cloud.ovh.net" }
use_path_style              = true
use_lockfile                = true
skip_credentials_validation = true
skip_region_validation      = true
skip_requesting_account_id  = true
skip_s3_checksum            = true
