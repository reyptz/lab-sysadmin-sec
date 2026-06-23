# Makefile du lab SysAdmin / SecOps / DevSecOps
# Sous Windows : utiliser Git Bash, WSL ou installer make via choco.

.PHONY: help monitoring-up monitoring-down onprem-init onprem-plan onprem-apply ansible-site cloud-validate cloud-plan checkov security-audit security-dfir security-malware security-yara security-portscan sre-check sre-apply chaos-pod-kill chaos-cpu-stress chaos-db-latency policies-kyverno policies-gatekeeper scan-trivy clean

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
# Infrastructure On-Premise (Proxmox + Ansible)
# ---------------------------------------------------------------
onprem-init: ## Initialise Terraform on-premise
	cd infrastructure/onprem/terraform && terraform init

onprem-plan: ## Plan Terraform on-premise
	cd infrastructure/onprem/terraform && terraform plan

onprem-apply: ## Applique Terraform on-premise
	cd infrastructure/onprem/terraform && terraform apply

ansible-site: ## Lance le playbook Ansible principal
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml site.yml

ansible-lvm: ## RHCSA — LVM, partitioning, fstab
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml lvm_storage.yml

ansible-firewalld: ## RHCSA — firewalld + nmcli networking
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml firewalld_network.yml

ansible-podman: ## RHCSA — Podman containers rootless
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml podman_containers.yml

ansible-cron-acls: ## RHCSA — cron/at, permissions, ACLs
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml cron_acls.yml

ansible-boot: ## RHCSA — boot targets, root reset, journalctl
	cd infrastructure/onprem/ansible && ansible-playbook -i inventory.yml boot_recovery.yml

# ---------------------------------------------------------------
# Cloud Multi-Provider
# ---------------------------------------------------------------
cloud-validate: ## Valide le format Terraform pour AWS, Azure et GCP
	@for d in infrastructure/cloud infrastructure/cloud/azure infrastructure/cloud/gcp; do \
		echo "==> $$d"; \
		cd $$d && terraform fmt -check && terraform validate; \
		cd - > /dev/null; \
	done

cloud-plan-aws: ## Plan Terraform AWS (Solutions Architect Professional)
	cd infrastructure/cloud && terraform plan -var="environment=dev"

cloud-plan-aws-sap: ## Plan Terraform AWS avec tous les features SAP actives
	cd infrastructure/cloud && terraform plan -var="environment=dev" -var="enable_config=true" -var="enable_guardduty=true" -var="enable_securityhub=true" -var="enable_waf=true" -var="enable_kinesis=true"

cloud-plan-azure: ## Plan Terraform Azure
	cd infrastructure/cloud/azure && terraform plan -var="environment=dev"

cloud-plan-azure-asae: ## Plan Terraform Azure avec tous les features ASAE actives
	cd infrastructure/cloud/azure && terraform plan -var="environment=dev" -var="enable_application_gateway=true" -var="enable_front_door=true" -var="enable_defender=true" -var="enable_sentinel=true" -var="enable_private_link=true"

cloud-plan-gcp: ## Plan Terraform GCP
	cd infrastructure/cloud/gcp && terraform plan -var="environment=dev" -var="gcp_project_id=$$GCP_PROJECT_ID"

cloud-plan-gcp-devops: ## Plan Terraform GCP avec tous les features Cloud DevOps actives
	cd infrastructure/cloud/gcp && terraform plan -var="environment=dev" -var="gcp_project_id=$$GCP_PROJECT_ID" -var="enable_cloud_deploy=true" -var="enable_cloud_run=true" -var="enable_cloud_functions=true" -var="enable_log_analytics=true"

# ---------------------------------------------------------------
# Securite & Qualite
# ---------------------------------------------------------------
checkov: ## Scan de securite des Terraform avec Checkov
	checkov -d infrastructure/cloud/ --framework terraform

# ---------------------------------------------------------------
# Blue / Red Team Tools
# ---------------------------------------------------------------
security-audit: ## Lance l'audit CIS local
	bash security/audit/cis_audit.sh

security-dfir: ## Collecte des preuves DFIR (CASE_ID=<id>)
	bash security/dfir/collection.sh $(CASE_ID)

security-malware: ## Analyse statique d'un echantillon (SAMPLE=<path>)
	bash security/malware_analysis/static_analysis.sh $(SAMPLE)

security-yara: ## Scan YARA sur un echantillon (SAMPLE=<path>)
	yara security/yara-rules/generic_malware.yar $(SAMPLE)
	yara security/yara-rules/suspicious_powershell.yar $(SAMPLE)

security-portscan: ## Scan reseau Red Team (TARGET=<ip/cidr>)
	python3 security/redteam/tools/port_scanner.py $(TARGET) --top-100

# ---------------------------------------------------------------
# SRE / Chaos / DevSecOps
# ---------------------------------------------------------------
sre-check: ## Verifie les regles Prometheus
	promtool check rules platform/sre/prometheus-rules.yml

sre-apply: ## Applique les regles Prometheus et le dashboard
	kubectl apply -f platform/sre/prometheus-rules.yml
	kubectl create configmap grafana-dashboard-invoices --from-file=platform/sre/grafana-dashboard.json -n monitoring --dry-run=client -o yaml | kubectl apply -f -

chaos-pod-kill: ## Tue un pod API invoices (Chaos Mesh)
	kubectl apply -f platform/chaos/experiments/pod-kill.yaml

chaos-cpu-stress: ## Stress CPU sur API invoices
	kubectl apply -f platform/chaos/experiments/cpu-stress.yaml

chaos-db-latency: ## Injecte de la latence DB
	kubectl apply -f platform/chaos/experiments/network-latency.yaml

policies-kyverno: ## Applique les policies Kyverno
	kubectl apply -f platform/devsecops/policies/kyverno/

policies-gatekeeper: ## Applique les templates et constraints Gatekeeper
	kubectl apply -f platform/devsecops/policies/gatekeeper/

scan-trivy: ## Scan local avec Trivy (IMAGE=<ref>)
	trivy image --config platform/devsecops/policies/trivy/trivy-config.yaml $(IMAGE)

clean: ## Supprime les fichiers temporaires
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .terraform -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name '*.retry' -delete 2>/dev/null || true
