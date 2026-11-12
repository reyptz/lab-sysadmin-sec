# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — Cost Optimization
# Savings Plans, Reserved Instances, Budgets
# ---------------------------------------------------------------------------

# AWS Budget (monthly cost)
resource "aws_budgets_budget" "monthly" {
  name              = "${var.project_name}-monthly-budget-${var.environment}"
  budget_type       = "COST"
  limit_amount      = var.monthly_budget
  limit_unit        = "USD"
  time_period_start = "2026-01-01_00:00"
  time_unit         = "MONTHLY"

  cost_filter {
    name = "TagKeyValue"
    # Format attendu par l'API Budgets : user:<clé>$<valeur>. format() évite
    # toute ambiguïté d'échappement du "$" dans un template HCL.
    values = [
      format("user:Project$%s", var.project_name),
    ]
  }

  # L'API Budgets refuse une notification sans abonné : on ne les crée que si
  # au moins un e-mail est fourni.
  dynamic "notification" {
    for_each = length(var.budget_alert_emails) > 0 ? [80, 100] : []

    content {
      comparison_operator        = "GREATER_THAN"
      threshold                  = notification.value
      threshold_type             = "PERCENTAGE"
      notification_type          = "ACTUAL"
      subscriber_email_addresses = var.budget_alert_emails
    }
  }
}

# EC2 Savings Plan (Compute Savings Plans)
# Savings Plans sont achetes via console/API ; Terraform supporte aws_savingsplans_plan via CLI credentials.
# On documente la recommandation.
resource "null_resource" "savings_plan_note" {
  triggers = {
    note = <<EOF
EC2 Savings Plans recommendation:
- Compute Savings Plans: 1-year, Partial Upfront, ~20-30% discount on EC2/Lambda/Fargate
- Apply to all regions, sizes, OS families
- Purchase via AWS Console -> Savings Plans -> Recommendations
- Use Cost Explorer to analyze On-Demand vs SP coverage
EOF
  }
}

# Reserved Capacity (DynamoDB): concept shown via provisioned capacity on app_sessions.
# In production, switch aws_dynamodb_table.app_sessions billing_mode to PROVISIONED
# and purchase Reserved Capacity via AWS Console / Cost Explorer.

# Cost Explorer / CUR : pas de resource Terraform native.
resource "null_resource" "cost_optimization_note" {
  triggers = {
    note = <<EOF
Cost optimization levers:
- Reserved Instances (RDS, EC2, ElastiCache) for steady-state workloads
- Spot Instances for stateless/fault-tolerant batch jobs
- S3 Intelligent-Tiering / Lifecycle policies
- Right-sizing via CloudWatch + Trusted Advisor
- Turn off dev/test resources nightly with Lambda/EventBridge
EOF
  }
}

# Scheduled dev shutdown (cost control via EventBridge + Lambda)
resource "aws_cloudwatch_event_rule" "shutdown_dev" {
  count               = var.environment == "dev" ? 1 : 0
  name                = "${var.project_name}-shutdown-dev-${var.environment}"
  description         = "Arret automatique des instances dev a 20h"
  schedule_expression = "cron(0 20 * * ? *)"
}

resource "aws_cloudwatch_event_target" "shutdown_lambda" {
  count = var.environment == "dev" ? 1 : 0
  rule  = aws_cloudwatch_event_rule.shutdown_dev[0].name
  arn   = aws_lambda_function.health_check.arn
}

resource "aws_lambda_permission" "allow_events" {
  count         = var.environment == "dev" ? 1 : 0
  statement_id  = "AllowExecutionFromEventBridge"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.health_check.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.shutdown_dev[0].arn
}
