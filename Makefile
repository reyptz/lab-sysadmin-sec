# Makefile du lab Cloud / DevOps / RHEL
# Sous Windows : utiliser Git Bash, WSL ou installer make via choco.

.PHONY: help monitoring-up monitoring-down ansible-site ansible-lvm ansible-firewalld ansible-podman ansible-cron-acls ansible-boot cloud-validate cloud-plan-aws cloud-plan-aws-sap checkov security-audit sre-check sre-apply policies-kyverno policies-gatekeeper scan-trivy clean

ALLOWED_SSH_CIDR ?= $(shell echo $$ALLOWED_SSH_CIDR)

help: ## Affiche l'aide
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ---------------------------------------------------------------
# Monitoring (Prometheus + Grafana)
# ---------------------------------------------------------------
monitoring-up: ## Demarre la stack monitoring
	docker compose -f platform/monitoring/docker-compose.yml up -d

monitoring-down: ## Arrete la stack monitoring
	docker compose -f platform/monitoring/docker-compose.yml down

# ---------------------------------------------------------------
# Ansible RHCSA
# ---------------------------------------------------------------
ansible-site: ## Lance le playbook Ansible principal
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml site.yml

ansible-lvm: ## RHCSA — LVM, partitioning, fstab
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml playbooks/lvm_storage.yml

ansible-firewalld: ## RHCSA — firewalld + nmcli networking
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml playbooks/firewalld_network.yml

ansible-podman: ## RHCSA — Podman containers rootless
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml playbooks/podman_containers.yml

ansible-cron-acls: ## RHCSA — cron/at, permissions, ACLs
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml playbooks/cron_acls.yml

ansible-boot: ## RHCSA — boot targets, root reset, journalctl
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory/inventory.yml playbooks/boot_recovery.yml

# ---------------------------------------------------------------
# Cloud AWS
# ---------------------------------------------------------------
cloud-validate: ## Valide le format Terraform pour AWS
	cd infrastructure/aws && terraform fmt -check && terraform init -backend=false && terraform validate

cloud-plan-aws: ## Plan Terraform AWS
	cd infrastructure/aws && terraform plan -var="environment=dev" -var="allowed_ssh_cidr=$(ALLOWED_SSH_CIDR)"

cloud-plan-aws-sap: ## Plan Terraform AWS avec tous les features SAP actives
	cd infrastructure/aws && terraform plan -var="environment=dev" -var="allowed_ssh_cidr=$(ALLOWED_SSH_CIDR)" -var="enable_config=true" -var="enable_guardduty=true" -var="enable_securityhub=true" -var="enable_waf=true" -var="enable_kinesis=true"

# ---------------------------------------------------------------
# Securite & Qualite
# ---------------------------------------------------------------
checkov: ## Scan de securite du module AWS avec Checkov
	checkov -d infrastructure/aws --framework terraform

security-audit: ## Lance l'audit CIS local
	bash security/audit/cis_audit.sh

# ---------------------------------------------------------------
# SRE / DevSecOps
# ---------------------------------------------------------------
sre-check: ## Verifie les regles Prometheus
	promtool check rules platform/sre/rules/prometheus-rules.yml

sre-apply: ## Applique les regles Prometheus et le dashboard
	kubectl apply -f platform/sre/rules/prometheus-rules.yml
	kubectl create configmap grafana-dashboard-invoices --from-file=platform/sre/dashboards/grafana-dashboard.json -n monitoring --dry-run=client -o yaml | kubectl apply -f -

policies-kyverno: ## Applique les policies Kyverno
	kubectl apply -f platform/devsecops/policies/kubernetes/kyverno/

policies-gatekeeper: ## Applique les templates et constraints Gatekeeper
	kubectl apply -f platform/devsecops/policies/kubernetes/gatekeeper/

scan-trivy: ## Scan local avec Trivy (IMAGE=<ref>)
	trivy image --config platform/devsecops/policies/scan/trivy/trivy-config.yaml $(IMAGE)

# ---------------------------------------------------------------
# Nettoyage
# ---------------------------------------------------------------
clean: ## Supprime les fichiers temporaires
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .terraform -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name '*.retry' -delete 2>/dev/null || true
