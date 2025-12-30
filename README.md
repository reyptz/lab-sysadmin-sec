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
├── ansible/            # Playbooks de configuration et hardening
│   ├── collections/    # Roles externes
│   ├── cis_benchmark.yml
│   ├── hardening.yml
│   ├── wazuh_agents.yml
│   └── ...
├── terraform/          # Définition de l'infrastructure virtuelle
├── architecture/       # Schémas et diagrammes (Visio/Draw.io)
├── wazuh/              # Règles et configuration SIEM
├── scripts/            # Scripts d'automatisation (Bash/Python)
├── firewall/           # Configurations OPNsense/pfSense
└── docker-compose.yml  # Stack de monitoring (Prometheus/Grafana)
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
   docker-compose up -d
   ```

3. **Provisionner l'Infra** (Exemple Terraform)
   ```bash
   cd terraform
   terraform init && terraform apply
   ```

4. **Configurer les noeuds**
   ```bash
   cd ../ansible
   ansible-playbook -i inventory.yml site.yml
   ```

---

## Auteur

Projet réalisé dans le cadre d'une montée en compétence **DevSecOps & SysAdmin**.

---
*Dernière mise à jour : Décembre 2025*