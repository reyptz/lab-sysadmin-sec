# Tests du Lab

Ce document centralise les tests de validation du projet.

## Types de tests prévus

- **Structure** : vérification que les dossiers obligatoires existent et que les fichiers de configuration sont bien placés.
- **Terraform** : `terraform validate` pour le module AWS (`infrastructure/aws`).
- **Ansible** : syntaxe des playbooks (`ansible-playbook --syntax-check`).
- **CI/CD** : exécution locale des étapes du workflow GitHub Actions (fmt, validate, checkov).

## Exemple de validation manuelle

```bash
# Toutes les commandes se lancent depuis la racine du dépôt.

# Terraform AWS
(cd infrastructure/aws && terraform fmt -check && terraform init -backend=false && terraform validate)

# Ansible (depuis le dossier qui contient ansible.cfg)
(cd infrastructure/onprem/ansible && ansible-galaxy collection install -r requirements.yml \
  && ansible-playbook -i inventory/inventory.yml --syntax-check site.yml)

# Application Node.js
(cd platform/cloudnative/app && npm ci --ignore-scripts && npm run lint && npm test)

# Overlays Kustomize (GitOps)
kubectl kustomize platform/gitops/argocd/apps/erp/overlays/prod-canary
```

> Sous Windows, utiliser Git Bash, WSL ou exécuter les commandes depuis un environnement Linux.
