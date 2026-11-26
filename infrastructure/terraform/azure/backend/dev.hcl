# État Terraform Azure (dev) : compte de stockage dédié, accès Entra ID.
resource_group_name  = "rg-lab-sysadmin-sec-tfstate"
storage_account_name = "labsysadminsectfstate"
container_name       = "tfstate"
key                  = "azure/dev/terraform.tfstate"
use_azuread_auth     = true
