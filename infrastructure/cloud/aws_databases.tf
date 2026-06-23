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
}

# RDS PostgreSQL multi-AZ
resource "aws_db_instance" "postgres" {
  identifier             = "${var.project_name}-postgres-${var.environment}"
  engine                 = "postgres"
  engine_version         = "16.1"
  instance_class         = "db.t3.micro"
  allocated_storage      = 20
  storage_type           = "gp3"
  storage_encrypted      = true
  multi_az               = true
  db_name                = "appdatabase"
  username               = "dbadmin"
  password               = var.db_password
  db_subnet_group_name   = aws_db_subnet_group.main.name
  vpc_security_group_ids = [aws_security_group.internal.id]
  parameter_group_name   = aws_db_parameter_group.main.name
  skip_final_snapshot    = true
  backup_retention_period = 7
  enabled_cloudwatch_logs_exports = ["postgresql"]

  tags = {
    Name = "${var.project_name}-postgres"
  }
}

# Aurora PostgreSQL cluster
resource "aws_rds_cluster" "aurora" {
  cluster_identifier      = "${var.project_name}-aurora-${var.environment}"
  engine                  = "aurora-postgresql"
  engine_version          = "15.4"
  database_name           = "auroradb"
  master_username         = "dbadmin"
  master_password         = var.db_password
  db_subnet_group_name    = aws_db_subnet_group.main.name
  vpc_security_group_ids  = [aws_security_group.internal.id]
  storage_encrypted       = true
  backup_retention_period = 7
  skip_final_snapshot     = true
}

resource "aws_rds_cluster_instance" "aurora_instances" {
  count               = 2
  identifier          = "${var.project_name}-aurora-${count.index + 1}-${var.environment}"
  cluster_identifier  = aws_rds_cluster.aurora.id
  instance_class      = "db.t3.medium"
  engine              = "aurora-postgresql"
  publicly_accessible = false
}

# DynamoDB table
resource "aws_dynamodb_table" "app_sessions" {
  name           = "${var.project_name}-sessions-${var.environment}"
  billing_mode     = "PAY_PER_REQUEST"
  hash_key       = "session_id"

  attribute {
    name = "session_id"
    type = "S"
  }

  point_in_time_recovery {
    enabled = true
  }

  server_side_encryption {
    enabled = true
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

# ElastiCache Redis cluster
resource "aws_elasticache_replication_group" "redis" {
  replication_group_id       = "${var.project_name}-redis-${var.environment}"
  description                = "Redis cluster for application caching"
  engine                     = "redis"
  engine_version             = "7.1"
  node_type                  = "cache.t3.micro"
  num_cache_clusters         = 2
  automatic_failover_enabled   = true
  multi_az_enabled           = true
  subnet_group_name          = aws_elasticache_subnet_group.redis.name
  security_group_ids         = [aws_security_group.internal.id]
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true

  tags = {
    Name = "${var.project_name}-redis"
  }
}
