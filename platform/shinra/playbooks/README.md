# Playbooks Shinra Defense

Définitions YAML des contre-mesures de la plateforme.

## Contenu

- `ransomware_response.yml` — réponse automatique à un ransomware
- `honeypot_provisioning.yml` — provisionnement des leurres
- `artifact_extraction.yml` — extraction et analyse des artefacts
- `network_isolation.yml` — isolation réseau des processus hostiles
- `system_hardening.yml` — durcissement post-incident

## Format

Ce ne sont **pas** des playbooks Ansible : `ansible-playbook` ne peut pas les
exécuter. Chaque fichier est un document déclaratif au schéma propre au projet :

- `name`, `description`, `version`, `author`
- `triggers` : conditions de déclenchement
- `actions` : contre-mesures à exécuter dans l'ordre
- `rollback` : actions de retour arrière

Aucun exécuteur de playbooks n'existe encore dans `src-engine/`. Les playbooks
Ansible du lab (durcissement RHEL, LVM, firewalld…) sont dans
`infrastructure/onprem/ansible/`.
