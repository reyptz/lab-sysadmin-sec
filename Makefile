# Makefile du lab Cloud / DevOps / RHEL
# Sous Windows : utiliser Git Bash, WSL ou installer make via choco.
#
# Les opérations de cycle de vie (déploiement, promotion, retour arrière,
# infrastructure, sauvegardes, correctifs, incidents) passent par socle :
# `make socle-help` ou `socle --help`. Les cibles ci-dessous couvrent la
# construction de l'outil, les contrôles locaux et le lab.

.PHONY: help socle-build socle-test socle-lint socle-validate socle-serve socle-help socle-image \
	monitoring-up monitoring-down ansible-deps ansible-site ansible-syntax security-audit \
	cloud-validate checkov sre-check sre-apply policies-kyverno scan-trivy gitops-render clean

SOCLE ?= socle/target/release/socle

help: ## Affiche l'aide
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

# ---------------------------------------------------------------
# socle — plan de contrôle du cycle de vie
# ---------------------------------------------------------------
socle-build: ## Compile socle (binaire optimisé)
	cargo build --release --locked --manifest-path socle/Cargo.toml -p socle

socle-test: ## Tests unitaires et d'intégration de socle
	cargo test --workspace --locked --manifest-path socle/Cargo.toml

socle-lint: ## Formatage et lint strict (identiques à la CI)
	cargo fmt --all --check --manifest-path socle/Cargo.toml
	cargo clippy --workspace --all-targets --locked --manifest-path socle/Cargo.toml -- -D warnings

socle-validate: socle-build ## Valide socle.yaml (références croisées)
	$(SOCLE) validate --spec socle.yaml

socle-serve: socle-build ## Serveur local (SQLite, console sur http://127.0.0.1:8080)
	$(SOCLE) serve --bind 127.0.0.1:8080

socle-help: socle-build ## Aide de la CLI socle
	$(SOCLE) --help

socle-image: ## Construit l'image conteneur de socle
	docker build -t socle:dev socle/

# ---------------------------------------------------------------
# Monitoring local (Prometheus + Grafana)
# ---------------------------------------------------------------
monitoring-up: ## Demarre la stack monitoring
	docker compose -f observability/docker-compose.yml up -d

monitoring-down: ## Arrete la stack monitoring
	docker compose -f observability/docker-compose.yml down

# ---------------------------------------------------------------
# Ansible (RHEL)
# ---------------------------------------------------------------
ansible-deps: ## Installe les collections Ansible requises
	cd infrastructure/ansible && ansible-galaxy collection install -r requirements.yml

ansible-site: ## Lance le playbook Ansible principal (RHCSA + durcissement)
	cd infrastructure/ansible && ansible-playbook -i inventory/inventory.yml site.yml

ansible-syntax: ## Verifie la syntaxe de tous les playbooks
	cd infrastructure/ansible && for pb in site.yml playbooks/*.yml; do ansible-playbook -i inventory/inventory.yml --syntax-check $$pb || exit 1; done

security-audit: ## Audit CIS en mode controle (aucune modification ; ecart = tache changed)
	cd infrastructure/ansible && ansible-playbook -i inventory/inventory.yml --check --diff playbooks/cis_benchmark.yml playbooks/hardening.yml

# ---------------------------------------------------------------
# Infrastructure (validation locale ; plan/application via socle)
# ---------------------------------------------------------------
cloud-validate: ## Valide toutes les stacks Terraform (AWS, Azure, GCP, OVHcloud, Scaleway)
	for s in aws azure gcp ovh scaleway; do (cd infrastructure/$$s && terraform fmt -check -recursive && terraform init -backend=false -input=false && terraform validate) || exit 1; done

checkov: ## Scan de securite du module AWS avec Checkov
	checkov -d infrastructure/terraform/aws --config-file infrastructure/terraform/aws/.checkov.yaml

# ---------------------------------------------------------------
# GitOps, SRE et politiques Kubernetes
# ---------------------------------------------------------------
gitops-render: ## Rend tous les overlays Kustomize (erreurs de manifests)
	for o in gitops/apps/erp/overlays/*/; do kubectl kustomize $$o > /dev/null || exit 1; done

sre-check: ## Verifie les regles Prometheus (SRE et socle)
	promtool check rules observability/rules/sre.yml observability/rules/socle.yml

# observability/rules/sre.yml est un fichier de regles Prometheus natif (pas un CRD
# PrometheusRule) : il est publie en ConfigMap, a monter dans rule_files.
sre-apply: ## Applique les regles Prometheus et le dashboard
	kubectl create configmap prometheus-rules-invoices --from-file=observability/rules/sre.yml -n monitoring --dry-run=client -o yaml | kubectl apply -f -
	kubectl create configmap grafana-dashboard-invoices --from-file=observability/dashboards/api-facturation.json -n monitoring --dry-run=client -o yaml | kubectl apply -f -

policies-kyverno: ## Applique les policies Kyverno (unique moteur d'admission)
	kubectl apply -f gitops/cluster/policies/

scan-trivy: ## Scan local avec Trivy (IMAGE=<ref>)
	trivy image --config security/trivy/trivy-config.yaml $(IMAGE)

# ---------------------------------------------------------------
# Nettoyage
# ---------------------------------------------------------------
clean: ## Supprime les fichiers temporaires
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .terraform -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name '*.retry' -delete 2>/dev/null || true
	find . -type f -name '.socle-*.tfplan' -delete 2>/dev/null || true
