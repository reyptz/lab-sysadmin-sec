# Sécurité

| Élément | Emplacement | Appliqué par |
|---|---|---|
| Portes de livraison (tests, vulnérabilités, signature, SBOM, ticket, fenêtres, quatre yeux) | `socle.yaml` (`policies`) | socle, à chaque opération |
| Politiques d'admission Kubernetes (Kyverno, unique moteur) | `gitops/cluster/policies/` | clusters (`make policies-kyverno`) |
| Analyse d'images | `security/trivy/trivy-config.yaml` | CI (`cicd.yml`, `shinra.yml`, `socle.yml`), rapport transmis à socle |
| Analyse IaC (Checkov) | `infrastructure/terraform/<stack>/.checkov.yaml` | CI `terraform-cloud.yml` |
| Secrets Kubernetes (External Secrets Operator : AWS SM ou Vault) | `gitops/cluster/secrets/` | clusters |
| Durcissement et audit CIS des serveurs RHEL | `infrastructure/ansible/playbooks/{hardening,cis_benchmark,selinux}.yml` | Ansible, runbook socle `audit-cis-rhel` |
| Détection d'intrusion hôte (OSSEC/Wazuh) | `security/ossec/` | serveurs RHEL |
| Défense active (leurres, eBPF) | `apps/shinra/` | socle (`shinra-engine`) |
