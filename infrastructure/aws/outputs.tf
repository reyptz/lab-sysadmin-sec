output "vpc_id" {
  description = "ID du VPC AWS"
  value       = aws_vpc.main.id
}

output "public_subnet_ids" {
  description = "IDs des subnets publics"
  value       = aws_subnet.public[*].id
}

output "private_subnet_ids" {
  description = "IDs des subnets privés"
  value       = aws_subnet.private[*].id
}

output "bastion_public_ip" {
  description = "IP publique du bastion"
  value       = aws_instance.bastion.public_ip
}

output "app_private_ips" {
  description = "IPs privées des serveurs applicatifs"
  value       = aws_instance.app[*].private_ip
}

output "backup_bucket_name" {
  description = "Nom du bucket S3 de backup"
  value       = aws_s3_bucket.backups.bucket
}

output "cloudwatch_log_group" {
  description = "Groupe de logs CloudWatch"
  value       = aws_cloudwatch_log_group.app_logs.name
}

output "rds_postgres_endpoint" {
  description = "Endpoint de l'instance RDS PostgreSQL"
  value       = aws_db_instance.postgres.endpoint
}

output "aurora_cluster_endpoint" {
  description = "Endpoint de l'ecrivain Aurora"
  value       = aws_rds_cluster.aurora.endpoint
}

output "dynamodb_table_name" {
  description = "Nom de la table DynamoDB"
  value       = aws_dynamodb_table.app_sessions.name
}

output "elasticache_redis_endpoint" {
  description = "Endpoint du cluster Redis ElastiCache"
  value       = aws_elasticache_replication_group.redis.primary_endpoint_address
}

output "lambda_function_name" {
  description = "Nom de la fonction Lambda"
  value       = aws_lambda_function.health_check.function_name
}

output "api_gateway_endpoint" {
  description = "Endpoint de l'API Gateway"
  value       = aws_apigatewayv2_api.main.api_endpoint
}

output "alb_dns_name" {
  description = "DNS name de l'ALB"
  value       = aws_lb.app.dns_name
}

output "ecs_cluster_name" {
  description = "Nom du cluster ECS"
  value       = aws_ecs_cluster.main.name
}

output "eks_cluster_name" {
  description = "Nom du cluster EKS"
  value       = aws_eks_cluster.main.name
}

output "cloudfront_domain_name" {
  description = "Domaine CloudFront (si domaine fourni)"
  value       = length(aws_cloudfront_distribution.cdn) > 0 ? aws_cloudfront_distribution.cdn[0].domain_name : ""
}

output "sns_topic_arn" {
  description = "ARN du topic SNS d'alertes"
  value       = aws_sns_topic.alerts.arn
}

output "sqs_queue_url" {
  description = "URL de la file SQS principale"
  value       = aws_sqs_queue.app_events.url
}

output "kinesis_stream_name" {
  description = "Nom du stream Kinesis (si active)"
  value       = length(aws_kinesis_stream.logs) > 0 ? aws_kinesis_stream.logs[0].name : ""
}
