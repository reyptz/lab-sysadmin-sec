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
| CI/CD multi-stages (build, tests, scans) | `.github/workflows/cicd.yml` + `.gitlab-ci.yml` | Terminé |
| GitOps (ArgoCD) | `platform/gitops/argocd/` | Terminé |
| Blue-Green / Canary | `platform/gitops/argocd/apps/erp/overlays/prod-canary/` | Terminé |
| Terraform AWS | `infrastructure/aws/` | Terminé |
| Ansible hardening / OS config | `infrastructure/onprem/ansible/` | Existant |
| Multi-tenant Kubernetes | `platform/cloudnative/k8s/tenancy/` | Terminé |
| Network Policies & RBAC | `platform/cloudnative/k8s/tenancy/` | Terminé |
| Gestion des secrets (ESO / Vault) | `platform/cloudnative/k8s/secrets/` | Terminé |
| Observabilité centralisée | `platform/monitoring/`, `platform/devsecops/` | Existant |
| SLI / SLO / Error Budgets | `platform/sre/` | Existant |

## Phases d'implémentation

### Phase 1 — Fondations (CI/CD + Tests)
- [x] Tests unitaires sur l'application (`platform/cloudnative/app/`).
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
