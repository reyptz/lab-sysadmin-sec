# GitOps avec ArgoCD

Ce dossier contient la configuration GitOps pour déployer l'ERP/CRM via ArgoCD.

## Principe

- Le dépôt Git est la **source de vérité**.
- ArgoCD surveille les manifests et applique automatiquement les changements.
- Le CI/CD met à jour le tag d'image (`images.newTag`) de l'overlay `dev` après un build réussi sur `main`.
- La promotion vers `staging` et `prod` se fait en reportant ce tag dans l'overlay correspondant, par pull request.
- `erp-prod` (Deployment) et `erp-prod-canary` (Argo Rollouts + Istio) ciblent le même namespace : ce sont deux alternatives, à ne pas activer simultanément.

## Structure

```bash
platform/gitops/argocd/
├── projects/
│   └── erp.yaml              # ArgoCD AppProject (isolation, RBAC)
├── apps/
│   └── erp/
│       ├── application.yaml  # Application ArgoCD
│       ├── base/             # Manifests communs
│       │   ├── kustomization.yaml
│       │   ├── deployment.yaml
│       │   └── service.yaml
│       └── overlays/
│           ├── dev/
│           ├── staging/
│           └── prod/
└── README.md
```

## Déploiement

```bash
# Installer ArgoCD (depuis la racine du dépôt)
kubectl create namespace argocd
kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Appliquer le projet et l'application
kubectl apply -f platform/gitops/argocd/projects/erp.yaml
kubectl apply -f platform/gitops/argocd/apps/erp/application.yaml
```

## Stratégies de déploiement

- **Blue-Green** : utiliser Argo Rollouts avec deux ReplicaSets.
- **Canary** : utiliser Argo Rollouts avec analysis et promotion progressive.

Voir les exemples dans `platform/gitops/argocd/apps/erp/overlays/`.
