terraform {
  # 1.10+ : verrouillage natif de l'état dans S3 (use_lockfile), sans DynamoDB.
  required_version = ">= 1.10.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
    archive = {
      source  = "hashicorp/archive"
      version = "~> 2.0"
    }
  }

  # État distant chiffré et verrouillé, un fichier de configuration par
  # environnement (backend/<env>.hcl) : `socle infra plan aws-core -e <env>`
  # passe automatiquement -backend-config. En validation hors ligne :
  # `terraform init -backend=false`.
  backend "s3" {}
}
