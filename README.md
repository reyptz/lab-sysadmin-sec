# Cloud DevOps RHEL Lab

![Status](https://img.shields.io/badge/Status-Active-success)
![Security](https://img.shields.io/badge/Security-Hardened-blue)
![IaC](https://img.shields.io/badge/IaC-Ansible%20%2B%20Terraform-orange)

## Présentation

Ce projet est un laboratoire d'infrastructure **Cloud**, **DevOps** et **RHEL**, conçu pour simuler un environnement d'entreprise réaliste et sécurisé. L'objectif est de déployer une infrastructure AWS hautement disponible et sécurisée, couplée à un socle RHEL automatisé via Ansible.

Il sert de démonstrateur technique pour des compétences avancées en administration système RHEL, ingénierie DevOps et architecture cloud AWS.

### Pilotage : socle

Tout le cycle de vie passe par **[socle](socle/README.md)**, le plan de contrôle du lab (Rust : CLI, API REST, console web) :
déploiements Kubernetes (GitOps) et VMs RHEL (Ansible), promotion d'artefacts immuables, portes DevSecOps
(tests, vulnérabilités, signature, SBOM), approbations à quatre yeux, retour arrière automatique,
infrastructure multi-cloud (Terraform/OpenTofu, plan approuvé = plan appliqué), sauvegardes avec RPO/RTO
et exercices de restauration, incidents, maintenance, journal d'audit infalsifiable et rapport de conformité
ISO 27001 / SOC 2. Source de vérité : [`socle.yaml`](socle.yaml).

```bash
make socle-build && socle validate          # spécification complète et cohérente
socle --local status                        # vue d'ensemble (ou SOCLE_URL + SOCLE_TOKEN vers le serveur)
socle promote erp-app -e prod --ticket CHG-1234 --dry-run
```

Documentation : [architecture](docs/socle/architecture.md) · [exploitation](docs/socle/operations.md).

---

## Architecture Technique

### Cloud AWS

| Composant | Rôle |
|:---|:---|
| **VPC multi-AZ** | Segmentation public / privé avec subnets sur 2 zones |
| **Bastion** | Accès SSH administratif restreint par CIDR |
| **ALB** | Répartiteur de charge applicatif public |
| **Serveurs applicatifs RHEL** | Instances EC2 privées avec hardening cloud-init |
| **EKS** | Cluster Kubernetes (déploiements GitOps Argo CD) |
| **Aurora PostgreSQL** | Base managée chiffrée, PITR (RPO ≤ 5 min), AWS Backup quotidien |
| **ElastiCache Redis** | Cache applicatif chiffré |
| **S3 backups** | Dépôts restic/Velero versionnés et chiffrés KMS (copies OVHcloud/Scaleway) |
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
- **Plan de contrôle socle** : un seul circuit (portes → approbations → exécution → vérification → retour arrière) pour tous les changements.
- **Provisioning** des ressources AWS via Terraform (état distant S3 verrouillé, plan approuvé puis appliqué par socle).
- **Configuration Management** RHEL avec Ansible (déploiement par lots, correctifs, sauvegardes restic/pgBackRest).
- **CI/CD** GitHub Actions : tests, scan, signature cosign, SBOM, release enregistrée dans socle.
- **GitOps** avec Argo CD (Argo Rollouts canary en production).

### Observabilité
- Métriques CloudWatch et alertes SNS.
- Stack Prometheus / Grafana locale via Docker Compose.
- Logs centralisés et règles SRE.

---

## Structure du Projet

```text
lab-sysadmin-sec/
├── socle.yaml            # Spécification de la plateforme (source de vérité de socle)
├── socle/                # Plan de contrôle (Rust) : CLI, API, console, moteur, déploiement HA
├── apps/
│   ├── erp/              # Application ERP (Node.js / Express, Dockerfile)
│   └── shinra/           # Défense active : moteur FastAPI (image), agent eBPF, UI, contre-mesures
├── gitops/
│   ├── apps/erp/         # Kustomize : base + overlays dev / staging / prod / prod-canary
│   ├── argocd/           # Applications et AppProject Argo CD
│   └── cluster/          # Ressources transverses : secrets (ESO), tenancy (RBAC, NetworkPolicies), policies (Kyverno)
├── infrastructure/
│   ├── terraform/        # Zones d'accueil : aws, azure, gcp, ovh, scaleway (env/, backend/ par environnement)
│   └── ansible/          # RHEL : RHCSA, durcissement/CIS, déploiement, correctifs, sauvegardes, scripts
├── observability/        # Prometheus, Alertmanager (→ incidents socle), règles, dashboards, stack locale
├── security/             # Vue d'ensemble DevSecOps, OSSEC/Wazuh, configuration Trivy
├── docs/                 # socle (architecture, exploitation), SRE, AWS, roadmap, tests
├── .github/              # CI/CD : application, socle, Terraform multi-cloud, Shinra
└── Makefile              # Raccourcis (build, tests, validations, lab)
```


## Installation & Démarrage

### Prérequis
- AWS CLI configuré avec les permissions nécessaires.
- Terraform >= 1.6.0.
- Ansible pour le socle RHEL.
- Docker & Docker Compose pour la stack monitoring.

### Déploiement AWS

```bash
export TF_VAR_allowed_ssh_cidr=<votre CIDR>     # variables sensibles : jamais versionnées
socle infra plan aws-core -e dev                 # état distant backend/dev.hcl, variables env/dev.tfvars
socle infra apply aws-core -e dev --plan-op <id-du-plan>
```

Validation hors ligne : `make cloud-validate`. L'état Terraform est distant (S3 chiffré, verrouillé) ;
voir l'amorçage dans [docs/socle/operations.md](docs/socle/operations.md).

### Démarrage Monitoring

```bash
make monitoring-up
# ou
docker compose -f observability/docker-compose.yml up -d
```

> Note : `GF_ADMIN_PASSWORD` doit être définie avant de démarrer Grafana.

### Configuration RHEL

```bash
cd infrastructure/ansible
ansible-galaxy collection install -r requirements.yml
export RHEL_CONTROL_HOST=<ip> RHEL_MANAGED_HOST=<ip> RHEL_ADMIN_USER=<user> ANSIBLE_PRIVATE_KEY_FILE=<chemin de la clé>
ansible-playbook -i inventory/inventory.yml site.yml
```

---

## RHCSA — Topics couverts (Ansible + Bash)

Les playbooks suivants couvrent les topics RHCSA pour l'infrastructure on-premise :

| Topic | Fichier / Commande |
|:---|:---|
| **SELinux enforcing + troubleshooting** | `infrastructure/ansible/playbooks/selinux.yml` |
| **SSH hardening, fail2ban, firewalld** | `infrastructure/ansible/playbooks/hardening.yml` |
| **Systemd services, dnf-automatic** | `playbooks/hardening.yml` |
| **User/group creation script** | `infrastructure/ansible/scripts/create_users.sh` |
| **LVM, partitioning, mount points, fstab** | `infrastructure/ansible/playbooks/lvm_storage.yml` |
| **firewalld, nmcli networking** | `infrastructure/ansible/playbooks/firewalld_network.yml` |
| **Podman rootless** | `infrastructure/ansible/playbooks/podman_containers.yml` |
| **cron/at, file permissions, ACLs** | `infrastructure/ansible/playbooks/cron_acls.yml` |
| **Boot targets, reset root password, journalctl** | `infrastructure/ansible/playbooks/boot_recovery.yml` |

---

## DevOps & SRE Enterprise

| Capacité | Implémentation |
|----------|----------------|
| **Plan de contrôle** | `socle/` + `socle.yaml` (CLI, API, console) |
| **CI/CD multi-stages** | `.github/workflows/cicd.yml` (signature, SBOM, release socle), `socle.yml` |
| **GitOps** | `gitops/` |
| **Blue-Green / Canary** | `gitops/apps/erp/overlays/prod-canary/` (piloté par socle) |
| **Sauvegardes RPO/RTO** | `socle.yaml` (backups) + `infrastructure/ansible/playbooks/backup_*.yml` |
| **Incidents & conformité** | `socle incident …`, `socle compliance` (ISO 27001 / SOC 2) |
| **Multi-tenant Kubernetes** | `gitops/cluster/tenancy/` |
| **Secrets management** | `gitops/cluster/secrets/` (ESO / Vault) |
| **SRE / SLI / SLO** | `docs/sre/` |
| **Défense active / honeypots (eBPF)** | `apps/shinra/` |

---

## Auteur

Projet réalisé dans le cadre d'une montée en compétence **Cloud DevOps RHEL**.

---
*Dernière mise à jour : Novembre 2026*