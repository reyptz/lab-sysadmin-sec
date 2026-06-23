# Référence Multi-Cloud du Lab SecOps

Ce dossier applique les compétences des certifications **AWS Solutions Architect Professional**, **Azure Solutions Architect Expert**, **GCP Professional Cloud DevOps Engineer** et **RHCSA** en proposant une version cloud-native de l'architecture on-premise `lab-sysadmin-sec`.

## Objectifs pédagogiques

- **AWS SAP-C02** : haute disponibilité multi-AZ, sécurité by design, résilience, optimisation des coûts (Well-Architected Framework).
- **Azure Solutions Architect** : identité hybride, gouvernance, segmentation réseau, monitoring centralisé.
- **GCP Cloud DevOps** : CI/CD, SRE, observabilité, infrastructure as code, GitOps.
- **RHCSA** : administration Linux, hardening, automatisation via cloud-init / Ansible.

## Structure

```bash
infrastructure/cloud/
├──               # Terraform AWS (VPC, EC2, IAM, CloudWatch, S3)
├── azure/        # Terraform Azure (VNet, VMs, NSG, Monitor, Storage)
├── gcp/          # Terraform GCP (VPC, Compute, IAM, Monitoring, GCS)
├── policies/     # Policy-as-Code (Checkov / OPA) pour la conformité
├── SRE.md        # SLIs / SLOs / runbooks
└── README.md
```

## Prérequis

- Terraform >= 1.6.0
- Compte AWS, Azure ou GCP configuré localement (CLI / credentials)
- Accès au backend Terraform (local par défaut, S3/GCS/Azure Blob recommandé en équipe)

## Démarrage rapide

### AWS
```bash
cd infrastructure/cloud/aws
terraform init
terraform plan -var="environment=prod"
terraform apply
```

### Azure
```bash
cd infrastructure/cloud/azure
terraform init
terraform plan -var="environment=prod"
terraform apply
```

### GCP
```bash
cd infrastructure/cloud/gcp
terraform init
terraform plan -var="environment=prod" -var="gcp_project_id=mon-projet-gcp"
terraform apply
```

## Architecture de référence

Chaque déploiement reproduit les couches du lab on-premise :

| Zone on-premise | Équivalent cloud |
|----------------|------------------|
| DMZ / Pare-feu | Security Groups / NSG / Firewall Rules |
| LAN serveurs | Subnets privés |
| SOC / Monitoring | CloudWatch / Azure Monitor / Cloud Monitoring + Log Analytics |
| Bastion SSH | Bastion host / AWS Systems Manager / Azure Bastion / IAP |
| SIEM Wazuh | Logs centralisés + règles de détection (fichier `wazuh-cloud.yml` à venir) |
| Backup / Logs | S3 / Azure Storage / GCS |

## Bonnes pratiques appliquées

- **Aucun secret en dur** : credentials et clés injectés via variables Terraform.
- **Moindre privilège** : rôles IAM/Azure AD/GCP Service Account limités au strict nécessaire.
- **Segmentation réseau** : subnets publics et privés avec flux restrictifs.
- **Haute disponibilité** : ressources réparties sur au moins 2 zones de disponibilité.
- **Observabilité** : métriques, logs et alertes activés nativement sur chaque cloud.
- **Policy-as-Code** : scan de sécurité via GitHub Actions (Checkov) avant déploiement.

## CI/CD

Le pipeline `.github/workflows/terraform-cloud.yml` exécute pour chaque pull request :
1. `terraform fmt -check`
2. `terraform validate`
3. Scan de sécurité avec Checkov
4. Plan Terraform pour AWS, Azure et GCP

## Couverture AWS Solutions Architect Professional

| Topic | Statut | Fichier Terraform |
|:---|:---:|:---|
| **VPC, EC2, S3, IAM, CloudWatch** | couvert | `main.tf` |
| **Multi-AZ, security groups, cloud-init hardening** | couvert | `main.tf`, `cloud-init.yml` |
| **Organizations, Control Tower, multi-account, SCPs** | couvert | `aws_organizations.tf` |
| **RDS/Aurora, DynamoDB, ElastiCache** | couvert | `aws_databases.tf` |
| **Lambda, API Gateway, Step Functions** | couvert | `aws_serverless.tf` |
| **ECS/EKS, Route53, CloudFront, ALB/NLB** | couvert | `aws_container_dns.tf` |
| **Migration (6Rs, DMS, Snow Family)** | couvert | `aws_migration.tf` |
| **Cost optimization (Savings Plans, RI, Budgets)** | couvert | `aws_cost_optimization.tf` |
| **Well-Architected, Config, GuardDuty, Security Hub** | couvert | `aws_security_compliance.tf` |
| **SQS, SNS, EventBridge, Kinesis** | couvert | `aws_messaging.tf` |

## Couverture Azure Solutions Architect Expert

| Topic | Statut | Fichier Terraform |
|:---|:---:|:---|
| **VNet, NSG, VMs, Managed Identity** | couvert | `infrastructure/cloud/azure/main.tf` |
| **Log Analytics, Storage Account, cloud-init** | couvert | `infrastructure/cloud/azure/main.tf` |
| **Entra ID, RBAC, Conditional Access** | couvert | `infrastructure/cloud/azure/azure_entra.tf` |
| **Management Groups, Azure Policy, Blueprints** | couvert | `infrastructure/cloud/azure/azure_governance.tf` |
| **AKS, App Service, Container Instances** | couvert | `infrastructure/cloud/azure/azure_containers.tf` |
| **SQL Database, Cosmos DB, Data Lake** | couvert | `infrastructure/cloud/azure/azure_databases.tf` |
| **Front Door, Application Gateway, Load Balancer** | couvert | `infrastructure/cloud/azure/azure_networking_services.tf` |
| **Site Recovery, Backup, geo-replication** | couvert | `infrastructure/cloud/azure/azure_recovery_backup.tf` |
| **Key Vault, Defender for Cloud, Sentinel** | couvert | `infrastructure/cloud/azure/azure_security.tf` |
| **ExpressRoute, VPN Gateway, Private Link** | couvert | `infrastructure/cloud/azure/azure_connectivity.tf` |

### Démarrage rapide AWS (tous les topics)

```bash
cd infrastructure/cloud
terraform init
terraform plan -var="enable_config=true" -var="enable_guardduty=true" -var="enable_securityhub=true" -var="enable_waf=true" -var="enable_kinesis=true"
terraform apply
```

### Démarrage rapide Azure (tous les topics)

```bash
cd infrastructure/cloud/azure
terraform init
terraform plan -var="enable_application_gateway=true" -var="enable_front_door=true" -var="enable_defender=true" -var="enable_sentinel=true" -var="enable_private_link=true"
terraform apply
```

## Couverture GCP Professional Cloud DevOps Engineer

| Topic | Statut | Fichier / Document |
|:---|:---:|:---|
| **SLI/SLO/SLA, error budgets, runbooks** | couvert | `infrastructure/cloud/SRE.md` |
| **CI/CD multi-stage (GitHub Actions + GitLab CI)** | couvert | `.github/workflows/`, `.gitlab-ci.yml` |
| **ArgoCD GitOps, canary rollouts, analysis** | couvert | `platform/gitops/argocd/` |
| **Cloud Monitoring, alerting, dashboard** | couvert | `infrastructure/cloud/gcp/main.tf` |
| **Incident response (NIST, MITRE ATT&CK)** | couvert | `docs/incident_response.md` |
| **IaC Terraform GCP (VPC, GCE, GCS, IAM)** | couvert | `infrastructure/cloud/gcp/main.tf` |
| **Cloud Build, Cloud Deploy, Artifact Registry** | couvert | `infrastructure/cloud/gcp/gcp_cicd.tf` |
| **GKE, Cloud Run, Cloud Functions** | couvert | `infrastructure/cloud/gcp/gcp_serverless_containers.tf` |
| **Cloud Logging avancé (sinks, exclusions, metrics)** | couvert | `infrastructure/cloud/gcp/gcp_logging.tf` |
| **Chaos engineering, postmortem culture** | couvert | `docs/gcp_chaos_engineering.md` |

### Démarrage rapide GCP (tous les topics)

```bash
cd infrastructure/cloud/gcp
terraform init
terraform plan -var="gcp_project_id=mon-projet-gcp" -var="enable_cloud_deploy=true" -var="enable_cloud_run=true" -var="enable_cloud_functions=true" -var="enable_log_analytics=true"
terraform apply
```

## Évolution possible

- Ajouter Kubernetes (EKS / AKS / GKE) pour migrer la stack CloudNative.
- Déployer Wazuh en cloud via un Auto Scaling Group / VMSS / MIG.
- Intégrer un backend Terraform partagé (S3 + DynamoDB, GCS + Cloud Build, Azure Blob + Storage Table).
- Ajouter un VPN / ExpressRoute / Cloud Interconnect pour un scénario hybride.
