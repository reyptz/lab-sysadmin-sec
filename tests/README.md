# Tests du Lab

Ce dossier centralise les tests de validation du projet.

## Types de tests prévus

- **Structure** : vérification que les dossiers obligatoires existent et que les fichiers de configuration sont bien placés.
- **Terraform** : `terraform validate` pour chaque provider cloud et l'on-premise.
- **Ansible** : syntaxe des playbooks (`ansible-playbook --syntax-check`).
- **CI/CD** : exécution locale des étapes du workflow GitHub Actions (fmt, validate, checkov).

## Exemple de validation manuelle

```bash
# Terraform cloud
for d in infrastructure/cloud/aws infrastructure/cloud/azure infrastructure/cloud/gcp; do
  echo "==> $d"
  (cd $d && terraform init -backend=false && terraform validate)
done

# Ansible
ansible-playbook -i infrastructure/onprem/ansible/inventory.yml --syntax-check infrastructure/onprem/ansible/site.yml
```

> Sous Windows, utiliser Git Bash, WSL ou exécuter les commandes depuis un environnement Linux.
