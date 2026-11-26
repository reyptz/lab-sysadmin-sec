# État Terraform de l'environnement prod (bucket versionné et chiffré, créé
# une fois hors Terraform : voir docs/socle/operations.md, « Amorçage »).
bucket       = "lab-sysadmin-sec-tfstate"
key          = "aws/prod/terraform.tfstate"
region       = "eu-west-3"
encrypt      = true
use_lockfile = true
