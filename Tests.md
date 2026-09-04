# Tests du Lab

Ce dossier centralise les tests de validation du projet.

## Types de tests prévus

- **Structure** : vérification que les dossiers obligatoires existent et que les fichiers de configuration sont bien placés.
- **Terraform** : `terraform validate` pour le module AWS (`infrastructure/aws`).
- **Ansible** : syntaxe des playbooks (`ansible-playbook --syntax-check`).
- **CI/CD** : exécution locale des étapes du workflow GitHub Actions (fmt, validate, checkov).

## Exemple de validation manuelle

```bash
# Terraform AWS
cd infrastructure/aws
terraform init -backend=false
terraform validate

# Ansible
ansible-playbook -i infrastructure/onprem/ansible/inventory/inventory.yml --syntax-check infrastructure/onprem/ansible/site.yml
```

> Sous Windows, utiliser Git Bash, WSL ou exécuter les commandes depuis un environnement Linux.
