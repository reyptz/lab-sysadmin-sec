# GitOps avec ArgoCD

Ce dossier contient la configuration GitOps pour déployer l'ERP/CRM via ArgoCD.

## Principe

- Le dépôt Git est la **source de vérité**.
- ArgoCD surveille les manifests et applique automatiquement les changements.
- **socle** fixe l'image par **digest** dans l'overlay (`images[].digest`) : la CI enregistre la release et la déploie en `dev` ; la promotion vers `staging` et `prod` passe par `socle promote` (portes, approbations, vérification, retour arrière automatique).
- Les bornes d'autoscaling de chaque overlay (`socle-scaling.yaml`) sont gérées par `socle scale`.
- Une modification manuelle d'un overlay est signalée comme **dérive** par `socle env`.
- `erp-prod` (Deployment) et `erp-prod-canary` (Argo Rollouts + Istio) ciblent le même namespace : ce sont deux alternatives, à ne pas activer simultanément.

## Structure

```text
gitops/
├── apps/erp/
│   ├── base/                 # Seule définition Kubernetes de l'ERP (Deployment, Service, HPA)
│   └── overlays/             # dev, staging, prod, prod-canary (+ socle-scaling.yaml géré par socle)
├── argocd/
│   ├── applications/         # Applications Argo CD (une par environnement)
│   └── projects/erp.yaml     # AppProject (isolation, RBAC)
└── cluster/
    ├── secrets/              # External Secrets Operator (AWS Secrets Manager ou Vault)
    ├── tenancy/              # Namespaces, RBAC, NetworkPolicies
    └── policies/             # Kyverno (unique moteur d'admission)
```

## Déploiement

```bash
# Installer ArgoCD (depuis la racine du dépôt)
kubectl create namespace argocd
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Projet puis applications (prod en canary, comme dans socle.yaml ;
# application.yaml = variante Deployment, ne pas activer en même temps)
kubectl apply -f gitops/argocd/projects/erp.yaml
kubectl apply -f gitops/argocd/applications/application-dev.yaml   -f gitops/argocd/applications/application-staging.yaml   -f gitops/argocd/applications/application-canary.yaml
```

## Stratégies de déploiement

- **Blue-Green** : utiliser Argo Rollouts avec deux ReplicaSets.
- **Canary** : utiliser Argo Rollouts avec analysis et promotion progressive.

Voir les exemples dans `gitops/apps/erp/overlays/`.
