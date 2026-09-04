# Cloud DevOps RHEL Lab

![Status](https://img.shields.io/badge/Status-Active-success)
![Security](https://img.shields.io/badge/Security-Hardened-blue)
![IaC](https://img.shields.io/badge/IaC-Ansible%20%2B%20Terraform-orange)

## Présentation

Ce projet est un laboratoire d'infrastructure **Cloud**, **DevOps** et **RHEL**, conçu pour simuler un environnement d'entreprise réaliste et sécurisé. L'objectif est de déployer une infrastructure AWS hautement disponible et sécurisée, couplée à un socle RHEL automatisé via Ansible.

Il sert de démonstrateur technique pour des compétences avancées en administration système RHEL, ingénierie DevOps et architecture cloud AWS.

---

## Architecture Technique

### Cloud AWS

| Composant | Rôle |
|:---|:---|
| **VPC multi-AZ** | Segmentation public / privé avec subnets sur 2 zones |
| **Bastion** | Accès SSH administratif restreint par CIDR |
| **ALB** | Répartiteur de charge applicatif public |
| **Serveurs applicatifs RHEL** | Instances EC2 privées avec hardening cloud-init |
| **RDS PostgreSQL / Aurora** | Bases de données managées, chiffrées |
| **ElastiCache Redis** | Cache applicatif chiffré |
| **S3 backups** | Stockage de sauvegardes avec versioning et chiffrement KMS |
| **CloudWatch / SNS** | Observabilité et alertes |
| **AWS Config / GuardDuty / Security Hub / WAF** | Conformité et sécurité (optionnels) |

### On-Premise / RHEL

| Rôle | Implémentation |
|:---|:---|
| **Hardening OS** | Playbooks Ansible (CIS, SELinux, firewalld, SSH) |
| **Stockage & réseau** | LVM, firewalld, nmcli, Podman |
| **Audit & HIDS** | CIS benchmarks, OSSEC |

---

## Stack Technologique

### Infrastructure as Code (IaC) & Configuration Management
![Terraform](https://img.shields.io/badge/Terraform-7B42BC?style=for-the-badge&logo=terraform&logoColor=white)
![Ansible](https://img.shields.io/badge/Ansible-EE0000?style=for-the-badge&logo=ansible&logoColor=white)

### Cloud Provider
![AWS](https://img.shields.io/badge/AWS-232F3E?style=for-the-badge&logo=amazon-aws&logoColor=white)

### Système & Conteneurisation
![Linux](https://img.shields.io/badge/RHEL-EE0000?style=for-the-badge&logo=redhat&logoColor=white)
![Docker](https://img.shields.io/badge/Docker-2496ED?style=for-the-badge&logo=docker&logoColor=white)

### Sécurité & Monitoring
![Wazuh](https://img.shields.io/badge/Wazuh-00B5E2?style=for-the-badge&logo=wazuh&logoColor=white)
![Grafana](https://img.shields.io/badge/Grafana-F46800?style=for-the-badge&logo=grafana&logoColor=white)
![Prometheus](https://img.shields.io/badge/Prometheus-E6522C?style=for-the-badge&logo=prometheus&logoColor=white)

---

## Fonctionnalités Clés

### Sécurité & Hardening
- **CIS Benchmarks** appliqués via Ansible.
- **Segmentation réseau** stricte par Security Groups (DMZ / LAN / données).
- **Bastion SSH** et **SSM** pour l'administration sécurisée.
- **IMDSv2** obligatoire sur les instances EC2.
- **Chiffrement** S3, RDS, ElastiCache et EBS.
- **Moindre privilège** IAM et Security Groups.

### Automatisation
- **Provisioning** des ressources AWS via Terraform.
- **Configuration Management** RHEL avec Ansible.
- **CI/CD** GitHub Actions pour Terraform et l'application ERP.
- **GitOps** avec ArgoCD.

### Observabilité
- Métriques CloudWatch et alertes SNS.
- Stack Prometheus / Grafana locale via Docker Compose.
- Logs centralisés et règles SRE.

---

## Structure du Projet

```bash
lab-sysadmin-sec/
├── .github/            # CI/CD GitHub Actions
├── docs/               # Documentation et roadmap
├── infrastructure/     # Infrastructure as Code (IaC)
│   ├── aws/            # Module Terraform AWS
│   └── onprem/
│       └── ansible/    # Playbooks RHCSA et hardening
│           ├── config/   # ansible.cfg et collections
│           ├── inventory/# Inventaire Ansible
│           ├── playbooks/# Playbooks RHCSA et hardening
│           └── site.yml  # Playbook principal
├── platform/          # Services, conteneurs et observabilité
│   ├── cloudnative/   # Application ERP et manifests Kubernetes
│   │   ├── app/     # Application Node.js conteneurisée
│   │   ├── helm/    # Chart Helm
│   │   └── k8s/     # Manifests Kubernetes
│   │       ├── app/      # Deployment et Service ERP
│   │       ├── secrets/  # Gestion des secrets
│   │       └── tenancy/  # Multi-tenancy, RBAC, NetworkPolicies
│   ├── devsecops/     # Stack SRE et policies
│   │   └── policies/   # Policies de sécurité
│   │       ├── iac/      # Checkov
│   │       ├── kubernetes/ # Kyverno, Gatekeeper
│   │       └── scan/     # Trivy
│   ├── gitops/        # ArgoCD
│   ├── monitoring/    # Docker Compose Prometheus/Grafana
│   └── sre/           # SLO/SLI, dashboards, runbooks
│       ├── dashboards/ # Dashboards Grafana
│       ├── rules/     # Règles Prometheus et SLO
│       └── runbooks/  # Runbooks opérationnels
├── security/          # Hardening RHEL
│   └── ossec/         # HIDS configuration
├── scripts/           # Scripts d'automatisation (Bash)
├── tests/             # Tests de validation
├── Makefile           # Commandes de gestion du lab
├── .gitignore
└── README.md
```

---

## Installation & Démarrage

### Prérequis
- AWS CLI configuré avec les permissions nécessaires.
- Terraform >= 1.6.0.
- Ansible pour le socle RHEL.
- Docker & Docker Compose pour la stack monitoring.

### Déploiement AWS

```bash
cd infrastructure/aws
cp terraform.tfvars.example terraform.tfvars  # ou exporter des TF_VAR_*
terraform init
terraform plan -var="allowed_ssh_cidr=<votre CIDR>"
terraform apply
```

Les variables sensibles (mot de passe DB, CIDR SSH) sont passées via variables Terraform ou `TF_VAR_*`. Aucun secret n'est versionné.

### Démarrage Monitoring

```bash
make monitoring-up
# ou
docker compose -f platform/monitoring/docker-compose.yml up -d
```

> Note : `GF_ADMIN_PASSWORD` doit être définie avant de démarrer Grafana.

### Configuration RHEL

```bash
cd infrastructure/onprem/ansible
ansible-playbook -i inventory/inventory.yml site.yml
```

---

## RHCSA — Topics couverts (Ansible + Bash)

Les playbooks suivants couvrent les topics RHCSA pour l'infrastructure on-premise :

| Topic | Fichier / Commande |
|:---|:---|
| **SELinux enforcing + troubleshooting** | `infrastructure/onprem/ansible/playbooks/selinux.yml` |
| **SSH hardening, fail2ban, firewalld** | `infrastructure/onprem/ansible/playbooks/hardening.yml` |
| **Systemd services, dnf-automatic** | `playbooks/hardening.yml` |
| **User/group creation script** | `scripts/create_users.sh` |
| **LVM, partitioning, mount points, fstab** | `infrastructure/onprem/ansible/playbooks/lvm_storage.yml` |
| **firewalld, nmcli networking** | `infrastructure/onprem/ansible/playbooks/firewalld_network.yml` |
| **Podman rootless** | `infrastructure/onprem/ansible/playbooks/podman_containers.yml` |
| **cron/at, file permissions, ACLs** | `infrastructure/onprem/ansible/playbooks/cron_acls.yml` |
| **Boot targets, reset root password, journalctl** | `infrastructure/onprem/ansible/playbooks/boot_recovery.yml` |

---

## DevOps & SRE Enterprise

| Capacité | Implémentation |
|----------|----------------|
| **CI/CD multi-stages** | `.github/workflows/cicd.yml` + `.gitlab-ci.yml` |
| **GitOps** | `platform/gitops/argocd/` |
| **Blue-Green / Canary** | `platform/gitops/argocd/apps/erp/overlays/prod-canary/` |
| **Multi-tenant Kubernetes** | `platform/cloudnative/k8s/tenancy/` |
| **Secrets management** | `platform/cloudnative/k8s/secrets/` (ESO / Vault) |
| **SRE / SLI / SLO** | `platform/sre/` |

---

## Auteur

Projet réalisé dans le cadre d'une montée en compétence **Cloud DevOps RHEL**.

---
*Dernière mise à jour : Septembre 2026*