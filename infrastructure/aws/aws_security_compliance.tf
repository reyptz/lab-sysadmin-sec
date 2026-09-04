# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — Security, Compliance, Well-Architected
# Config, GuardDuty, Security Hub, WAF
# ---------------------------------------------------------------------------

# AWS Config — recorder + delivery channel
resource "aws_config_configuration_recorder" "main" {
  count = var.enable_config ? 1 : 0
  name     = "${var.project_name}-config-recorder-${var.environment}"
  role_arn = aws_iam_role.config[0].arn

  recording_group {
    all_supported                 = true
    include_global_resource_types = true
  }
}

resource "aws_config_delivery_channel" "main" {
  count          = var.enable_config ? 1 : 0
  name           = "${var.project_name}-config-delivery-${var.environment}"
  s3_bucket_name = aws_s3_bucket.config[0].bucket
  depends_on     = [aws_config_configuration_recorder.main[0]]
}

resource "aws_config_configuration_recorder_status" "main" {
  count  = var.enable_config ? 1 : 0
  name   = aws_config_configuration_recorder.main[0].name
  is_enabled = true
  depends_on = [aws_config_delivery_channel.main[0]]
}

resource "aws_s3_bucket" "config" {
  count  = var.enable_config ? 1 : 0
  bucket = "${var.project_name}-config-${var.environment}-${random_id.bucket_suffix.hex}"
}

resource "aws_iam_role" "config" {
  count = var.enable_config ? 1 : 0
  name  = "${var.project_name}-config-role-${var.environment}"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = { Service = "config.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "config" {
  count      = var.enable_config ? 1 : 0
  role       = aws_iam_role.config[0].name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSConfigRole"
}

# Config Rule: S3 bucket public read prohibited
resource "aws_config_config_rule" "s3_public_read" {
  count = var.enable_config ? 1 : 0
  name  = "${var.project_name}-s3-bucket-public-read-prohibited-${var.environment}"

  source {
    owner             = "AWS"
    source_identifier = "S3_BUCKET_PUBLIC_READ_PROHIBITED"
  }

  depends_on = [aws_config_configuration_recorder.main[0]]
}

# GuardDuty
resource "aws_guardduty_detector" "main" {
  count = var.enable_guardduty ? 1 : 0
  enable = true
}

# Security Hub
resource "aws_securityhub_account" "main" {
  count = var.enable_securityhub ? 1 : 0
}

resource "aws_securityhub_standards_association" "cis" {
  count = var.enable_securityhub ? 1 : 0
  depends_on = [aws_securityhub_account.main[0]]
  standards_arn = "arn:aws:securityhub:${var.aws_region}::standards/cis-aws-foundations-benchmark/v/3.0.0"
}

resource "aws_securityhub_standards_association" "foundational" {
  count = var.enable_securityhub ? 1 : 0
  depends_on = [aws_securityhub_account.main[0]]
  standards_arn = "arn:aws:securityhub:${var.aws_region}::standards/aws-foundational-security-best-practices/v/1.0.0"
}

# AWS WAF (Web ACL) associe au CloudFront / ALB
resource "aws_wafv2_web_acl" "main" {
  count = var.enable_waf ? 1 : 0
  name        = "${var.project_name}-waf-${var.environment}"
  description = "WAF ACL basique pour le lab"
  scope       = "REGIONAL"

  default_action {
    allow {}
  }

  rule {
    name     = "AWSManagedRulesCommonRuleSet"
    priority = 1

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesCommonRuleSet"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "AWSManagedRulesCommonRuleSetMetric"
      sampled_requests_enabled   = true
    }
  }

  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "${var.project_name}-waf-metric"
    sampled_requests_enabled   = true
  }
}

resource "aws_wafv2_web_acl_association" "alb" {
  count        = var.enable_waf ? 1 : 0
  resource_arn = aws_lb.app.arn
  web_acl_arn  = aws_wafv2_web_acl.main[0].arn
}

# AWS Well-Architected Tool — workloads via console ; on documente.
resource "null_resource" "well_architected_note" {
  triggers = {
    note = <<EOF
AWS Well-Architected Framework (6 pillars):
1. Operational Excellence
2. Security
3. Reliability
4. Performance Efficiency
5. Cost Optimization
6. Sustainability

Process: definir un workload dans AWS Well-Architected Tool, repondre aux questions, identifier les high-risk issues.
EOF
  }
}
