# Enterprise SecOps & SysAdmin Lab

![Status](https://img.shields.io/badge/Status-Active-success)
![Security](https://img.shields.io/badge/Security-Hardened-blue)
![IaC](https://img.shields.io/badge/IaC-Ansible%20%2B%20Terraform-orange)

## Présentation

Ce projet est un laboratoire complet d'infrastructure système et sécurité, conçu pour simuler un environnement d'entreprise réaliste. L'objectif est de déployer une infrastructure **hautement disponible**, **sécurisée "by design"**, et entièrement **automatisée**.

Il sert de démonstrateur technique pour des compétences avancées en administration système (SysAdmin), ingénierie DevOps et cybersécurité (SOC/Blue Team).

---

## Architecture Technique

L'infrastructure est segmentée pour reproduire les contraintes réelles de sécurité (DMZ, LAN, SOC).

### Topologie Réseau
| Service | VLAN | Rôle |
|:---|:---:|:---|
| **Utilisateurs** | `10` | Postes clients (Windows 10) |
| **Serveurs** | `20` | Services internes (AD, DNS, Apps) |
| **SOC / Securité** | `99` | Monitoring, SIEM (Wazuh), Audit |

### Rôles des Machines
- **OPNsense / pfSense** : Pare-feu périmétrique et segmentation réseau (VLANs).
- **Windows Server 2022** : Cœur de l'identité (AD DS, DNS, DHCP, GPO).
- **Debian 13** : Infrastructure Core (DNS Sec, Bastion SSH, Syslog, NTP).
- **Ubuntu Server** : Plateforme applicative conteneurisée (Docker, Reverse Proxy).
- **Wazuh Server** : SIEM centralisé pour la détection d'intrusions.
- **Kali Linux / Parrot** : Audit offensif (Red Team).

---

## Stack Technologique

Le projet s'appuie sur des outils standards de l'industrie :

### Infrastructure as Code (IaC) & Config Management
![Terraform](https://img.shields.io/badge/Terraform-7B42BC?style=for-the-badge&logo=terraform&logoColor=white)
![Ansible](https://img.shields.io/badge/Ansible-EE0000?style=for-the-badge&logo=ansible&logoColor=white)
![Packer](https://img.shields.io/badge/Packer-02A8EF?style=for-the-badge&logo=packer&logoColor=white)

### Cloud Providers
![AWS](https://img.shields.io/badge/AWS-232F3E?style=for-the-badge&logo=amazon-aws&logoColor=white)
![Azure](https://img.shields.io/badge/Azure-0078D4?style=for-the-badge&logo=microsoft-azure&logoColor=white)
![Google Cloud](https://img.shields.io/badge/Google_Cloud-4285F4?style=for-the-badge&logo=google-cloud&logoColor=white)

### Système & Conteneurisation
![Windows](https://img.shields.io/badge/Windows_Server-0078D6?style=for-the-badge&logo=windows&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![Docker](https://img.shields.io/badge/Docker-2496ED?style=for-the-badge&logo=docker&logoColor=white)

### Sécurité & Monitoring
![Wazuh](https://img.shields.io/badge/Wazuh-00B5E2?style=for-the-badge&logo=wazuh&logoColor=white)
![Grafana](https://img.shields.io/badge/Grafana-F46800?style=for-the-badge&logo=grafana&logoColor=white)
![Prometheus](https://img.shields.io/badge/Prometheus-E6522C?style=for-the-badge&logo=prometheus&logoColor=white)

---

## Fonctionnalités Clés

### Sécurité & Hardening
- **CIS Benchmarks** appliqués via Ansible.
- **Segmentation réseau** stricte par VLANs.
- **Reverse Proxy** (Nginx) pour l'exposition des services web.
- **Bastion SSH** pour l'administration sécurisée.

### Automatisation
- **Provisioning** des VMs via Terraform.
- **Configuration Management** complet avec Ansible (Roles & Collections).
- **Golden Images** générées avec Packer.

### Observabilité (SIEM & Logs)
- Centralisation des logs systèmes et applicatifs.
- Détection d'attaques en temps réel (Brute force, Lateral movement).
- Dashboards de visualisation Grafana.

---

## Structure du Projet

```bash
lab-sysadmin-sec/
├── .github/            # CI/CD GitHub Actions
├── docs/               # Documentation et schemas d'architecture
│   ├── architecture/   # Diagrammes (Draw.io/Visio)
│   └── incident_response.md
├── infrastructure/     # Infrastructure as Code (IaC)
│   ├── onprem/         # Proxmox, Ansible, Packer, Firewall
│   │   ├── terraform/
│   │   ├── ansible/
│   │   ├── packer/
│   │   └── firewall/
│   └── cloud/          # Architectures de reference AWS / Azure / GCP
│       ├── aws/
│       ├── azure/
│       ├── gcp/
│       ├── policies/   # Policy-as-Code (Checkov)
│       ├── SRE.md      # SLIs / SLOs / runbooks
│       └── README.md
├── platform/           # Services, conteneurs et observabilite
│   ├── cloudnative/    # Docker, Kubernetes, Helm
│   │   ├── app/        # Application ERP/CRM (Node.js + tests)
│   │   ├── k8s/        # Manifests Kubernetes
│   │   │   ├── tenancy/      # Multi-tenant : namespaces, RBAC, NetworkPolicies
│   │   │   └── secrets/      # ESO / Vault
│   │   └── helm/
│   ├── devsecops/      # Stack SRE complete (Prometheus, Grafana, Loki, etc.)
│   │   └── policies/   # Gatekeeper, Kyverno, Trivy, Checkov
│   ├── gitops/         # ArgoCD : applications, projets, overlays
│   ├── consul/         # Service discovery / mesh
│   ├── monitoring/     # Docker Compose monitoring leger (Prometheus/Grafana)
│   ├── sre/            # SLO/SLI, error budget, dashboards, runbooks
│   ├── chaos/          # Chaos Engineering experiments
│   ├── platform-engineering/ # Backstage, Crossplane
│   └── mlops/          # KServe, pipelines, drift, AIOps
├── security/           # Outils et regles de securite
│   ├── sigma-rules/    # Detection as Code
│   ├── yara-rules/     # Analyse statique malware
│   ├── ossec/          # HIDS configuration
│   ├── dfir/           # Response a incident
│   ├── redteam/        # OSINT et outils Red Team
│   ├── malware_analysis/ # Analyse statique
│   ├── grc/            # Gouvernance, risque, conformite
│   ├── audit/          # Audit CIS Benchmarks
│   └── threat-intel/   # Threat intelligence
├── scripts/            # Scripts d'automatisation (Bash/Python)
├── tests/              # Tests d'integration et validation
├── Makefile            # Commandes de gestion du lab
├── .gitignore
└── README.md
```

---

## Scénarios de Démonstration (Use Cases)

Ce lab permet de simuler et analyser des attaques réelles :

### Blue Team vs Red Team
1. **Intrusion SSH** : Tentative de Brute Force depuis Kali → Détection Wazuh → Ban automatique via Active Response.
2. **Escalade de privilèges** : Modification fichiers critiques → Alerte intégrité FIM (File Integrity Monitoring).
3. **Mouvement Latéral** : Détection de trafic suspect entre VLANs via les logs Firewall.

---

## Installation & Démarrage

### Prérequis
- Hyperviseur (VMware Workstation, Proxmox ou VirtualBox).
- Docker & Docker Compose (sur la machine de management).
- Ansible & Terraform installés.

### Déploiement Rapide

1. **Cloner le dépôt**
   ```bash
   git clone https://github.com/reyptz/lab-sysadmin-sec.git
   cd lab-sysadmin-sec
   ```

2. **Démarrer le Monitoring**
   ```bash
   docker compose -f platform/monitoring/docker-compose.yml up -d
   # ou via Makefile : make monitoring-up
   ```

3. **Provisionner l'Infra** (Exemple Terraform)
   ```bash
   cd infrastructure/onprem/terraform
   terraform init && terraform apply
   ```

4. **Configurer les noeuds**
   ```bash
   cd ../../onprem/ansible
   ansible-playbook -i inventory.yml site.yml
   ```

---

## Sécurité — Blue Team / Red Team / GRC

Le dossier `security/` centralise les règles, scripts et procédures de sécurité.

| Capacité | Fichier | Commande |
|---|---|---|
| Règles Sigma | `security/sigma-rules/` | `sigma convert -t wazuh security/sigma-rules/linux_auth_bruteforce.yml` |
| Règles YARA | `security/yara-rules/` | `make security-yara SAMPLE=<path>` |
| OSSEC HIDS | `security/ossec/` | `sudo cp security/ossec/ossec.conf /var/ossec/etc/` |
| Audit CIS | `security/audit/cis_audit.sh` | `make security-audit` |
| DFIR collection | `security/dfir/collection.sh` | `make security-dfir CASE_ID=<id>` |
| Analyse statique malware | `security/malware_analysis/static_analysis.sh` | `make security-malware SAMPLE=<path>` |
| Red Team OSINT | `security/redteam/osint/` | — |
| Port scanner | `security/redteam/tools/port_scanner.py` | `make security-portscan TARGET=<ip/cidr>` |
| GRC | `security/grc/` | — |

Voir le mapping complet des compétences dans [`docs/skills_matrix.md`](./docs/skills_matrix.md).

---

## SRE, Platform Engineering, MLOps & DevSecOps

| Capacité | Fichier | Commande |
|---|---|---|
| SLO / SLI / Error Budget | `platform/sre/` | `promtool check rules platform/sre/prometheus-rules.yml` |
| Dashboards SRE | `platform/sre/grafana-dashboard.json` | `kubectl create configmap grafana-dashboard-invoices --from-file=platform/sre/grafana-dashboard.json` |
| Chaos Engineering | `platform/chaos/experiments/` | `kubectl apply -f platform/chaos/experiments/pod-kill.yaml` |
| Game Day | `platform/chaos/game-day.md` | — |
| On-call / Runbooks | `platform/sre/runbooks/` | — |
| Backstage templates | `platform/platform-engineering/backstage/` | — |
| Crossplane compositions | `platform/platform-engineering/crossplane/` | `kubectl apply -f platform/platform-engineering/crossplane/compositions/` |
| KServe inference | `platform/mlops/kserve/` | `kubectl apply -f platform/mlops/kserve/inference-service.yaml` |
| MLOps pipelines | `platform/mlops/pipelines/` | Argo Workflows |
| DevSecOps policies | `platform/devsecops/policies/` | `kubectl apply -f platform/devsecops/policies/kyverno/` |
| Leadership / Toil | `docs/leadership/` | — |

---

## RHCSA — Topics On-Premise (Ansible + Bash)

Les playbooks suivants couvrent l'intégralité du programme **RHCSA (Red Hat Certified System Administrator)** pour l'infrastructure on-premise :

| Topic | Statut | Fichier / Commande |
|:---|:---:|:---|
| **SELinux enforcing + troubleshooting** | couvert | `infrastructure/onprem/ansible/selinux.yml` |
| **SSH hardening, fail2ban, UFW** | couvert | `infrastructure/onprem/ansible/hardening.yml` |
| **NTP/Chrony, DNS BIND9, Samba** | couvert | `ntp.yml`, `bind9.yml`, `samba_debian.yml` |
| **Systemd services, unattended-upgrades** | couvert | `hardening.yml` |
| **User/group creation script** | couvert | `scripts/create_users.sh` |
| **LVM, partitioning, mount points, fstab** | couvert | `infrastructure/onprem/ansible/lvm_storage.yml` |
| **firewalld (pas UFW), nmcli networking** | couvert | `infrastructure/onprem/ansible/firewalld_network.yml` |
| **Podman containers (pas Docker)** | couvert | `infrastructure/onprem/ansible/podman_containers.yml` |
| **cron/at jobs, file permissions, ACLs** | couvert | `infrastructure/onprem/ansible/cron_acls.yml` |
| **Boot targets, reset root password, journalctl** | couvert | `infrastructure/onprem/ansible/boot_recovery.yml` |

### Lancement rapide par topic

```bash
# LVM + fstab
make ansible-lvm

# firewalld + nmcli
make ansible-firewalld

# Podman rootless
make ansible-podman

# cron / at / ACLs
make ansible-cron-acls

# Boot targets + journalctl
make ansible-boot

# Tout le lab
make ansible-site
```

---

## Extension Cloud (AWS / Azure / GCP)

Ce projet inclut une **architecture de référence multi-cloud** en Terraform, couvrant les compétences des certifications cibles :

- **AWS Solutions Architect Professional** : VPC multi-AZ, IAM moindre privilège, CloudWatch, S3 chiffré, RDS/Aurora, DynamoDB, Lambda/API Gateway, ECS/EKS, CloudFront, Route53, DMS, Security Hub, GuardDuty, Config, SQS/SNS/EventBridge, Kinesis, Cost Optimization, Organizations/SCPs.
- **Azure Solutions Architect Expert** : VNet, NSG, Azure Monitor, Managed Identities, Stockage chiffré, Entra ID RBAC, Conditional Access, Management Groups, Azure Policy, AKS, App Service, SQL Database, Cosmos DB, Data Lake, Front Door, Application Gateway, Backup/Site Recovery, Key Vault, Defender for Cloud, Sentinel, ExpressRoute, VPN Gateway, Private Link.
- **GCP Professional Cloud DevOps Engineer** : VPC, firewall, IAM, Cloud Monitoring, SRE, CI/CD GitHub Actions, Cloud Build, Cloud Deploy, Artifact Registry, GKE, Cloud Run, Cloud Functions, Cloud Logging avancé, Chaos Engineering, Postmortem Culture.
- **RHCSA** : cloud-init hardening Linux, fail2ban, audit, SSH sécurisé.

Voir le dossier [`infrastructure/cloud/`](./infrastructure/cloud/) et le [`infrastructure/cloud/README.md`](./infrastructure/cloud/README.md) pour les instructions détaillées.

### CI/CD

Le pipeline `.github/workflows/terraform-cloud.yml` valide automatiquement le format Terraform, lance `terraform validate`, exécute un scan de sécurité Checkov, et génère un plan pour chaque cloud sur les pull requests.

---

## DevOps & SRE Enterprise

Cette partie du lab applique les pratiques enterprise pour une application critique dans un contexte bancaire/fintech.

| Capacité | Implémentation |
|----------|----------------|
| **CI/CD multi-stages** | `.github/workflows/erp-cicd.yml` + `.gitlab-ci.yml` (lint, tests, build, scan Trivy, push GHCR/GitLab Registry) |
| **GitOps** | `platform/gitops/argocd/` — applications ArgoCD, projets, overlays dev/staging/prod |
| **Blue-Green / Canary** | `platform/gitops/argocd/apps/erp/overlays/prod-canary/` avec Argo Rollouts |
| **Multi-tenant Kubernetes** | `platform/cloudnative/k8s/tenancy/` (namespaces, RBAC, NetworkPolicies) |
| **Secrets management** | `platform/cloudnative/k8s/secrets/` (External Secrets Operator + HashiCorp Vault) |
| **SRE / SLI / SLO** | `infrastructure/cloud/SRE.md` avec error budgets et PromQL |

### Démarrage rapide de l'application

```bash
# Tests locaux
cd platform/cloudnative/app
npm ci
npm test

# Build image
docker build -t erp-app:latest .

# Déployer via ArgoCD
kubectl apply -f platform/gitops/argocd/projects/erp.yaml
kubectl apply -f platform/gitops/argocd/apps/erp/application.yaml
```

Voir la roadmap complète dans [`docs/roadmap.md`](./docs/roadmap.md).

---

## Auteur

Projet réalisé dans le cadre d'une montée en compétence **DevSecOps & SysAdmin**.

---
*Dernière mise à jour : Juin 2026*