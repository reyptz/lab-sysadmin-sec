# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — RDS/Aurora, DynamoDB, ElastiCache
# ---------------------------------------------------------------------------

# Subnet group pour les bases de donnees (multi-AZ)
resource "aws_db_subnet_group" "main" {
  name       = "${var.project_name}-db-subnet-group-${var.environment}"
  subnet_ids = aws_subnet.private[*].id

  tags = {
    Name = "${var.project_name}-db-subnet-group"
  }
}

# Groupe de parametres pour l'audit
resource "aws_db_parameter_group" "main" {
  name   = "${var.project_name}-pg-${var.environment}"
  family = "postgres16"

  parameter {
    name  = "log_connections"
    value = "1"
  }

  parameter {
    name  = "log_disconnections"
    value = "1"
  }

  # Chiffrement en transit obligatoire.
  parameter {
    name  = "rds.force_ssl"
    value = "1"
  }
}

# Cluster Aurora : journalisation des requetes + SSL obligatoire.
resource "aws_rds_cluster_parameter_group" "aurora" {
  name   = "${var.project_name}-aurora-pg-${var.environment}"
  family = "aurora-postgresql15"

  parameter {
    name  = "log_statement"
    value = "ddl"
  }

  parameter {
    name  = "log_min_duration_statement"
    value = "1000"
  }

  parameter {
    name         = "rds.force_ssl"
    value        = "1"
    apply_method = "immediate"
  }
}

# Role pour la supervision avancee (Enhanced Monitoring) RDS/Aurora.
resource "aws_iam_role" "rds_monitoring" {
  name = "${var.project_name}-rds-monitoring-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "monitoring.rds.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "rds_monitoring" {
  role       = aws_iam_role.rds_monitoring.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonRDSEnhancedMonitoringRole"
}

# RDS PostgreSQL
# engine_version ne fixe que la version majeure : AWS retire régulièrement les
# versions mineures, un patch figé (ex. 16.1) finit par faire échouer la création.
resource "aws_db_instance" "postgres" {
  identifier                          = "${var.project_name}-postgres-${var.environment}"
  engine                              = "postgres"
  engine_version                     = "16"
  auto_minor_version_upgrade          = true
  copy_tags_to_snapshot               = true
  # db.t3.small minimum : Performance Insights n'est pas disponible sur micro.
  instance_class                      = "db.t3.small"
  allocated_storage                   = 20
  storage_type                        = "gp3"
  storage_encrypted                   = true
  kms_key_id                          = aws_kms_key.platform.arn
  multi_az                            = var.environment == "prod"
  deletion_protection                 = var.environment == "prod"
  monitoring_interval                 = 60
  monitoring_role_arn                 = aws_iam_role.rds_monitoring.arn
  performance_insights_enabled        = true
  performance_insights_kms_key_id     = aws_kms_key.platform.arn
  publicly_accessible                 = false
  iam_database_authentication_enabled = true
  db_name                             = "appdatabase"
  username                            = "dbadmin"
  password                            = local.db_password
  db_subnet_group_name                = aws_db_subnet_group.main.name
  vpc_security_group_ids              = [aws_security_group.internal.id]
  parameter_group_name                = aws_db_parameter_group.main.name
  skip_final_snapshot                 = true
  backup_retention_period             = 7
  enabled_cloudwatch_logs_exports     = ["postgresql"]

  tags = {
    Name = "${var.project_name}-postgres"
  }
}

# Aurora PostgreSQL cluster
resource "aws_rds_cluster" "aurora" {
  cluster_identifier                  = "${var.project_name}-aurora-${var.environment}"
  engine                              = "aurora-postgresql"
  engine_version                      = "15.4"
  database_name                       = "auroradb"
  master_username                     = "dbadmin"
  master_password                     = local.db_password
  db_subnet_group_name                = aws_db_subnet_group.main.name
  vpc_security_group_ids              = [aws_security_group.internal.id]
  storage_encrypted                   = true
  kms_key_id                          = aws_kms_key.platform.arn
  db_cluster_parameter_group_name     = aws_rds_cluster_parameter_group.aurora.name
  iam_database_authentication_enabled = true
  copy_tags_to_snapshot               = true
  backup_retention_period             = 7
  skip_final_snapshot                 = true
  deletion_protection                 = var.environment == "prod"
  enabled_cloudwatch_logs_exports     = ["postgresql"]
}

resource "aws_rds_cluster_instance" "aurora_instances" {
  count                      = 2
  identifier                 = "${var.project_name}-aurora-${count.index + 1}-${var.environment}"
  cluster_identifier         = aws_rds_cluster.aurora.id
  instance_class             = "db.t3.medium"
  engine                     = "aurora-postgresql"
  publicly_accessible        = false
  auto_minor_version_upgrade = true

  monitoring_interval             = 60
  monitoring_role_arn             = aws_iam_role.rds_monitoring.arn
  performance_insights_enabled    = true
  performance_insights_kms_key_id = aws_kms_key.platform.arn
}

# DynamoDB table
resource "aws_dynamodb_table" "app_sessions" {
  name         = "${var.project_name}-sessions-${var.environment}"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "session_id"

  attribute {
    name = "session_id"
    type = "S"
  }

  point_in_time_recovery {
    enabled = true
  }

  server_side_encryption {
    enabled     = true
    kms_key_arn = aws_kms_key.platform.arn
  }

  tags = {
    Name = "${var.project_name}-sessions"
  }
}

# ElastiCache Redis subnet group
resource "aws_elasticache_subnet_group" "redis" {
  name       = "${var.project_name}-redis-subnet-${var.environment}"
  subnet_ids = aws_subnet.private[*].id
}

# Auth token Redis (le chiffrement en transit exige un token côté client de toute façon).
resource "random_password" "redis_auth" {
  length  = 32
  special = false # certains clients Redis rejettent les caractères spéciaux dans l'AUTH
}

locals {
  redis_auth_token = var.redis_auth_token != "" ? var.redis_auth_token : random_password.redis_auth.result
}

# ElastiCache Redis cluster
resource "aws_elasticache_replication_group" "redis" {
  replication_group_id       = "${var.project_name}-redis-${var.environment}"
  description                = "Redis cluster for application caching"
  engine                     = "redis"
  engine_version             = "7.1"
  node_type                  = "cache.t3.micro"
  num_cache_clusters         = 2
  automatic_failover_enabled = true
  multi_az_enabled           = true
  subnet_group_name          = aws_elasticache_subnet_group.redis.name
  security_group_ids         = [aws_security_group.internal.id]
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  kms_key_id                 = aws_kms_key.platform.arn
  auth_token                 = local.redis_auth_token

  tags = {
    Name = "${var.project_name}-redis"
  }
}

# ---------------------------------------------------------------------------
# AWS Backup : sauvegarde quotidienne des bases (RDS + Aurora)
# ---------------------------------------------------------------------------
resource "aws_backup_vault" "main" {
  name        = "${var.project_name}-vault-${var.environment}"
  kms_key_arn = aws_kms_key.platform.arn
}

resource "aws_backup_plan" "databases" {
  name = "${var.project_name}-db-backup-${var.environment}"

  rule {
    rule_name         = "daily"
    target_vault_name = aws_backup_vault.main.name
    schedule          = "cron(0 3 * * ? *)"

    lifecycle {
      delete_after = 35
    }
  }
}

resource "aws_iam_role" "backup" {
  name = "${var.project_name}-backup-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "backup.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "backup" {
  role       = aws_iam_role.backup.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSBackupServiceRolePolicyForBackup"
}

resource "aws_backup_selection" "databases" {
  name         = "${var.project_name}-db-selection-${var.environment}"
  iam_role_arn = aws_iam_role.backup.arn
  plan_id      = aws_backup_plan.databases.id

  resources = [
    aws_db_instance.postgres.arn,
    aws_rds_cluster.aurora.arn,
  ]
}
