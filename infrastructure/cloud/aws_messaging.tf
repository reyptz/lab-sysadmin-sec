# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — Messaging & Eventing
# SQS, SNS, EventBridge, Kinesis
# ---------------------------------------------------------------------------

# SQS queues
resource "aws_sqs_queue" "app_events" {
  name                        = "${var.project_name}-app-events-${var.environment}"
  visibility_timeout_seconds  = 180
  message_retention_seconds   = 86400
  receive_wait_time_seconds   = 10
  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.app_events_dlq.arn
    maxReceiveCount     = 3
  })

  tags = {
    Name = "${var.project_name}-app-events"
  }
}

resource "aws_sqs_queue" "app_events_dlq" {
  name = "${var.project_name}-app-events-dlq-${var.environment}"

  tags = {
    Name = "${var.project_name}-app-events-dlq"
  }
}

# SNS topic
resource "aws_sns_topic" "alerts" {
  name = "${var.project_name}-alerts-${var.environment}"

  tags = {
    Name = "${var.project_name}-alerts"
  }
}

resource "aws_sns_topic_subscription" "email" {
  count     = length(var.alert_emails)
  topic_arn = aws_sns_topic.alerts.arn
  protocol  = "email"
  endpoint  = var.alert_emails[count.index]
}

# EventBridge bus + rule
resource "aws_cloudwatch_event_bus" "custom" {
  name = "${var.project_name}-bus-${var.environment}"
}

resource "aws_cloudwatch_event_rule" "ec2_state_change" {
  name        = "${var.project_name}-ec2-state-change-${var.environment}"
  description = "Capture EC2 state changes"
  event_bus_name = aws_cloudwatch_event_bus.custom.name

  event_pattern = jsonencode({
    source      = ["aws.ec2"]
    detail-type = ["EC2 Instance State-change Notification"]
  })
}

resource "aws_cloudwatch_event_target" "sns" {
  rule           = aws_cloudwatch_event_rule.ec2_state_change.name
  event_bus_name = aws_cloudwatch_event_bus.custom.name
  arn            = aws_sns_topic.alerts.arn
}

# Kinesis Data Stream
resource "aws_kinesis_stream" "logs" {
  count       = var.enable_kinesis ? 1 : 0
  name        = "${var.project_name}-logs-${var.environment}"
  shard_count = 1

  encryption_type = "KMS"

  tags = {
    Name = "${var.project_name}-logs"
  }
}

# Kinesis Firehose delivery stream to S3
resource "aws_kinesis_firehose_delivery_stream" "logs" {
  count       = var.enable_kinesis ? 1 : 0
  name        = "${var.project_name}-logs-firehose-${var.environment}"
  destination = "extended_s3"

  extended_s3_configuration {
    role_arn   = aws_iam_role.firehose[0].arn
    bucket_arn = aws_s3_bucket.backups.arn
    prefix     = "logs/"
  }

  tags = {
    Name = "${var.project_name}-logs-firehose"
  }
}

resource "aws_iam_role" "firehose" {
  count = var.enable_kinesis ? 1 : 0
  name  = "${var.project_name}-firehose-${var.environment}"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = { Service = "firehose.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy" "firehose_s3" {
  count = var.enable_kinesis ? 1 : 0
  name  = "${var.project_name}-firehose-s3-${var.environment}"
  role  = aws_iam_role.firehose[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["s3:PutObject", "s3:PutObjectAcl"]
      Resource = "${aws_s3_bucket.backups.arn}/logs/*"
    }]
  })
}
