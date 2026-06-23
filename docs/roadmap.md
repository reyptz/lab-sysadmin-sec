# Roadmap — Capacités Cloud & SRE

Ce document formalise l'application des capacités DevOps / Cloud / SRE requises pour une application dans un contexte sécurisé, en les mappant sur la structure du lab.

## Objectifs métier

- Déployer une application critique avec **zero downtime**.
- Garantir l'**isolation multi-tenant** des données clients.
- Sécuriser les secrets (clés API IA, credentials BDD) hors du code.
- Mesurer et maintenir la **fiabilité** via des SLI/SLO et des budgets d'erreur.
- Automatiser le build, les tests, la sécurité et le déploiement.

## Mapping des capacités

| Capacité | Fichier / Dossier cible | Statut |
|----------|------------------------|--------|
| CI/CD multi-stages (build, tests, scans) | `.github/workflows/cicd.yml` + `.gitlab-ci.yml` | Terminé |
| GitOps (ArgoCD) | `platform/gitops/argocd/` | Terminé |
| Blue-Green / Canary | `platform/gitops/argocd/apps/app/overlays/prod-canary/` | Terminé |
| Terraform multi-environnement | `infrastructure/cloud/{aws,azure,gcp}/` | En cours |
| Ansible hardening / OS config | `infrastructure/onprem/ansible/` (existant) | Existant |
| Multi-tenant Kubernetes | `platform/cloudnative/k8s/tenancy/` | Terminé |
| Network Policies & RBAC | `platform/cloudnative/k8s/tenancy/` | Terminé |
| Gestion des secrets (ESO / Vault) | `platform/cloudnative/k8s/secrets/` | Terminé |
| Observabilité centralisée | `platform/monitoring/`, `platform/devsecops/` | Existant |
| SLI / SLO / Error Budgets | `infrastructure/cloud/SRE.md` | Terminé |

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

### Phase 4 — Multi-Cloud & SRE
- [ ] Terraform modules réutilisables par environnement pour AWS/Azure/GCP.
- [x] SLI/SLO et Error Budgets définis et monitorés.
- [x] Alerting et runbooks de production.

## Phase 5 — Blue Team / Red Team / GRC

- [x] Règles Sigma pour SSH brute-force, escalation Windows, lateral movement.
- [x] Règles YARA pour malwares génériques et PowerShell suspect.
- [x] Configuration OSSEC HIDS.
- [x] Hardening Windows Server (Kerberos, WSUS, RBAC).
- [x] Scripts d'audit CIS Benchmarks.
- [x] Procédures et scripts DFIR.
- [x] Outils Red Team (OSINT, port scanner).
- [x] Documents GRC (risk assessment, CCM).
- [x] Threat intelligence sources.

## Phase 6 — Platform Engineering, AIOps, DevSecOps & Leadership

- [x] SRE concret : SLO/SLI, error budget, burn rate alerts pour API /invoices.
- [x] Chaos Engineering : pod kill, CPU stress, latence DB, game day.
- [x] On-call mature : runbooks, rotation, postmortems.
- [x] Backstage : template "New Microservice".
- [x] Crossplane : composition PostgreSQL managée.
- [x] MLOps : pipeline Argo, KServe inference, canary 10 %.
- [x] AIOps : détection d'anomalies et corrélation.
- [x] DevSecOps : policies Gatekeeper, Kyverno, Trivy, Checkov.
- [x] Leadership : mentoring, RFC, toil tracker.

## Contraintes fintech prises en compte

- **RGPD / PCI-DSS** : chiffrement des données au repos et en transit, logs d'audit, isolation des tenants.
- **Résilience** : replicas multiples, probes, HPA, PDB.
- **Sécurité** : pas de secret en dur, scans d'images, least privilege, Network Policies.
- **Auditabilité** : versioning Git, CI/CD traçable, dashboards de performance.
