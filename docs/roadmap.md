# Roadmap — Capacités Cloud DevOps RHEL

Ce document formalise l'application des capacités Cloud, DevOps et RHEL requises pour le lab.

## Objectifs métier

- Déployer une infrastructure AWS sécurisée et résiliente.
- Garantir l'isolation réseau et la moindre privilège (IAM, Security Groups).
- Sécuriser les secrets (credentials BDD, tokens) hors du code.
- Mesurer et maintenir la fiabilité via des SLI/SLO et des budgets d'erreur.
- Automatiser le build, les tests, la sécurité et le déploiement.

## Mapping des capacités

| Capacité | Fichier / Dossier cible | Statut |
|----------|------------------------|--------|
| Plan de contrôle du cycle de vie (CLI, API, console) | `socle/`, `socle.yaml` | Terminé |
| CI/CD multi-stages (build, tests, scans, signature, SBOM) | `.github/workflows/cicd.yml`, `socle.yml` | Terminé |
| GitOps (ArgoCD) | `gitops/` | Terminé |
| Blue-Green / Canary | `gitops/apps/erp/overlays/prod-canary/` | Terminé |
| Terraform AWS | `infrastructure/terraform/aws/` | Terminé |
| Ansible hardening / OS config | `infrastructure/ansible/` | Existant |
| Multi-tenant Kubernetes | `gitops/cluster/tenancy/` | Terminé |
| Network Policies & RBAC | `gitops/cluster/tenancy/` | Terminé |
| Gestion des secrets (ESO / Vault) | `gitops/cluster/secrets/` | Terminé |
| Observabilité centralisée | `observability/` | Terminé |
| SLI / SLO / Error Budgets | `docs/sre/` + vérification SLO post-déploiement (socle) | Terminé |
| Sauvegardes RPO 15 min / RTO 1 h, exercices | `socle.yaml` (backups), playbooks restic/pgBackRest | Terminé |
| Incidents, maintenance, conformité ISO 27001 / SOC 2 | `socle` | Terminé |

## Phases d'implémentation

### Phase 1 — Fondations (CI/CD + Tests)
- [x] Tests unitaires sur l'application (`apps/erp/`).
- [x] Pipeline GitHub Actions : lint, tests, build image, scan de sécurité (Trivy/Snyk).
- [x] Publication d'image dans un registre (GitHub Container Registry par défaut).

### Phase 2 — GitOps & Déploiement Avancé
- [x] Application ArgoCD pour l'application.
- [x] Manifests Blue-Green ou Canary (Argo Rollouts).
- [x] Séparation des environnements `dev` / `staging` / `prod`.

### Phase 3 — Multi-Tenancy & Sécurité
- [x] Namespaces dédiés par tenant / environnement.
- [x] Network Policies restrictives entre microservices.
- [x] RBAC avec moindre privilège pour les équipes et les service accounts.
- [x] External Secrets Operator ou Vault pour les secrets.

### Phase 4 — Cloud AWS & SRE
- [x] Module Terraform AWS : VPC, IAM, EC2, RDS, S3, CloudWatch, sécurité.
- [x] SLI/SLO et Error Budgets définis et monitorés.
- [x] Alerting et runbooks de production.

### Phase 5 — Hardening RHEL
- [x] Scripts d'audit CIS Benchmarks.
- [x] Configuration OSSEC HIDS.
- [x] Playbooks Ansible RHCSA (SELinux, firewalld, Podman, LVM, etc.).

## Contraintes prises en compte

- **Sécurité** : chiffrement des données au repos et en transit, logs d'audit, moindre privilège.
- **Résilience** : multi-AZ, backups, CloudWatch, alertes.
- **Opérabilité** : pas de secret en dur, scans d'images, CI/CD traçable.

### Phase 6 — Plan de contrôle et consolidation
- [x] socle : modèle déclaratif, portes DevSecOps, quatre yeux, retour arrière automatique.
- [x] Suppression des doublons (ECS, RDS, Helm, manifests bruts, GitLab CI, Gatekeeper, scripts de sauvegarde et d'audit).
- [x] État Terraform distant et verrouillé ; application du seul plan approuvé.

### Phase 7 — Suites
- [x] Zones d'accueil Terraform Azure, GCP, OVHcloud, Scaleway, déclarées dans `socle.yaml` et validées en CI.
- [x] Exercice de restauration Aurora automatisé (clone PITR mesuré) et contrôle continu du RPO.
- [x] Image Shinra testée, signée et déployée par socle (`shinra-engine`), scripts ad hoc retirés.
- [ ] Checkov bloquant sur les nouvelles zones d'accueil ; agent eBPF Shinra en unité systemd.

