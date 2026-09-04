# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — Organizations, Control Tower, SCPs
# ---------------------------------------------------------------------------

# Note: AWS Organizations doit etre cree manuellement via la console.
# Ce fichier suppose que l'organization existe et injecte son ID via variable.

resource "aws_organizations_policy" "deny_root_account" {
  count   = var.organization_id != "" ? 1 : 0
  name    = "DenyRootAccountUsage"
  type    = "SERVICE_CONTROL_POLICY"
  content = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect    = "Deny"
        Action    = "*"
        Resource  = "*"
        Condition = {
          StringLike = {
            "aws:PrincipalArn" = ["arn:aws:iam::*:root"]
          }
        }
      }
    ]
  })
}

resource "aws_organizations_policy" "require_imds_v2" {
  count   = var.organization_id != "" ? 1 : 0
  name    = "RequireIMDSv2"
  type    = "SERVICE_CONTROL_POLICY"
  content = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Deny"
        Action   = "ec2:RunInstances"
        Resource = "arn:aws:ec2:*:*:instance/*"
        Condition = {
          StringEquals = {
            "ec2:MetadataHttpTokens" = "optional"
          }
        }
      }
    ]
  })
}

resource "aws_organizations_organizational_unit" "workloads" {
  count     = var.organization_id != "" ? 1 : 0
  name      = "Workloads-${var.environment}"
  parent_id = var.organization_root_id
}

resource "aws_organizations_organizational_unit" "security" {
  count     = var.organization_id != "" ? 1 : 0
  name      = "Security-${var.environment}"
  parent_id = var.organization_root_id
}

# Control Tower n'a pas de provider Terraform officiel a ce jour.
# On documente l'activation via console et l'usage de Account Factory.
resource "null_resource" "control_tower_note" {
  count = var.organization_id != "" ? 1 : 0
  triggers = {
    note = <<EOF
Control Tower doit etre active manuellement depuis la console AWS.
- Landing Zone : region principale + region de dr
- Account Factory : creation standardisee des comptes Workloads / Security
- Guardrails : enabled via SCPs et detectives via Config Rules
EOF
  }
}
