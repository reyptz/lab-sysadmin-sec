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

# Reserved Capacity (DynamoDB): concept shown via provisioned capacity on app_sessions.
# In production, switch aws_dynamodb_table.app_sessions billing_mode to PROVISIONED
# and purchase Reserved Capacity via AWS Console / Cost Explorer.

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
