# AWS Migration Runbook — Stratégie 6Rs

Ce runbook documente la stratégie de migration vers AWS pour le lab `lab-sysadmin-sec`.

## 1. Les 6Rs

| Stratégie | Description | Cas d'usage dans le lab |
|:---|:---|:---|
| **Rehost** | Lift-and-shift : VM on-premise -> EC2 | Serveurs applicatifs, bastion |
| **Replatform** | Leger changement sans refonte code | Base de données MySQL/PostgreSQL -> RDS/Aurora via DMS |
| **Refactor** | Réarchitecture en microservices / cloud-native | Monolithe ERP -> conteneurs ECS/EKS |
| **Repurchase** | Remplacement par un SaaS / managed service | SIEM Wazuh on-premise -> service cloud-native |
| **Retain** | Garder en local | Legacy incompatible ou non rentable à migrer |
| **Retire** | Arrêter le service | Applications non utilisées |

## 2. Outils de migration

- **AWS Application Migration Service (MGN)** : lift-and-shift automatisé.
- **AWS Database Migration Service (DMS)** : migration continue (CDC) des bases relationnelles.
- **AWS Snow Family** : transfert physique pour gros volumes ou connectivité limitée.
  - **Snowcone** : 8 TB, edge computing.
  - **Snowball Edge** : 80 TB, transfert sécurisé.
  - **Snowmobile** : exabyte-scale, datacenter entier.
- **AWS Migration Hub** : suivi centralisé de l'avancement.
- **AWS Transfer Family / DataSync** : transfert de fichiers en ligne.

## 3. Séquence type

1. **Découverte & évaluation** : AWS Application Discovery Service + TCO.
2. **Conception cible** : VPC multi-AZ, subnets, security groups, IAM.
3. **Migration pilote** : rehost d'une application non critique.
4. **Migration données** : DMS avec validation (AWS SCT pour conversions de schéma).
5. **Cutover** : bascule DNS (Route53), tests, rollback plan.
6. **Optimisation** : right-sizing, Reserved Instances/Savings Plans, automation.

## 4. Exemple DMS

```bash
# Terraform
cd infrastructure/aws
export TF_VAR_db_password="$(< /dev/urandom tr -dc 'A-Za-z0-9' | head -c 24)"
terraform apply -var="enable_dms=true" -var="db_password=$TF_VAR_db_password"
```

Points de vigilance :
- Activer les logs CloudWatch sur DMS.
- Valider la cohérence des données (row count + checksums).
- Planifier la maintenance des connexions source/cible.

## 5. Sécurité pendant la migration

- Chiffrement en transit (TLS/SSL) et au repos (KMS/SSE).
- Réseau privé via AWS Direct Connect ou VPN.
- Moindre privilège : rôles IAM dédiés à DMS/MGN.
- Monitoring via CloudWatch et GuardDuty.
