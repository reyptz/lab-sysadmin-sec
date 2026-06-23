# ---------------------------------------------------------------------------
# AWS Solutions Architect Professional — Migration (6Rs, DMS, Snow Family)
# ---------------------------------------------------------------------------

# Database Migration Service (DMS) — replication instance + endpoints
resource "aws_dms_replication_instance" "main" {
  count                = var.enable_dms ? 1 : 0
  replication_instance_id    = "${var.project_name}-dms-${var.environment}"
  replication_instance_class   = "dms.t3.micro"
  allocated_storage            = 20
  engine_version               = "3.5.1"
  publicly_accessible            = false
  replication_subnet_group_id  = aws_dms_replication_subnet_group.main[0].id
  vpc_security_group_ids       = [aws_security_group.internal.id]

  tags = {
    Name = "${var.project_name}-dms"
  }
}

resource "aws_dms_replication_subnet_group" "main" {
  count       = var.enable_dms ? 1 : 0
  replication_subnet_group_id = "${var.project_name}-dms-subnet-${var.environment}"
  subnet_ids  = aws_subnet.private[*].id

  tags = {
    Name = "${var.project_name}-dms-subnet"
  }
}

# DMS endpoints (placeholders : remplacer par les vrais moteurs/credentials)
resource "aws_dms_endpoint" "source" {
  count         = var.enable_dms ? 1 : 0
  endpoint_id   = "${var.project_name}-source-${var.environment}"
  endpoint_type = "source"
  engine_name   = "mysql"
  username      = "sourceuser"
  password      = var.db_password
  server_name   = "source-db.example.com"
  port          = 3306
  database_name = "sourcedb"
}

resource "aws_dms_endpoint" "target" {
  count         = var.enable_dms ? 1 : 0
  endpoint_id   = "${var.project_name}-target-${var.environment}"
  endpoint_type = "target"
  engine_name   = "postgres"
  username      = "dbadmin"
  password      = var.db_password
  server_name   = aws_db_instance.postgres.address
  port          = 5432
  database_name = "appdatabase"
}

# Snow Family : pas de ressource Terraform directe ; on documente.
resource "null_resource" "snow_family_note" {
  triggers = {
    note = <<EOF
AWS Snow Family (migration physique) :
- Snowcone : 8 TB, edge computing leger
- Snowball Edge : 80 TB, transfert securise vers S3
- Snowmobile : exabyte-scale, datacenter complet
Use cases : 6Rs Retain/Rehost/Repurchase/Replatform/Refactor/Retire
EOF
  }
}

# Migration Hub : pas de provider Terraform officiel.
# On cree un document de runbook dans le repertoire docs.
resource "null_resource" "migration_6rs_note" {
  triggers = {
    note = <<EOF
Strategie 6Rs pour le lab :
1. Rehost (lift-and-shift) : VMs on-premise -> EC2 via AWS MGN
2. Replatform : bases de donnees -> RDS/Aurora via DMS
3. Refactor : monolithe -> microservices ECS/EKS
4. Repurchase : SIEM on-premise -> SaaS/cloud-native
5. Retain : legacy maintenu en local
6. Retire : services non utilises arretes
EOF
  }
}
