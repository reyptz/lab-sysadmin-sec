# Tests du Lab

Toutes les commandes se lancent depuis la racine du dépôt ; la CI
(`.github/workflows/socle.yml`, `cicd.yml`, `terraform-cloud.yml`) exécute les mêmes.

## socle (plan de contrôle)

```bash
make socle-lint                 # cargo fmt --check + clippy -D warnings
make socle-test                 # tests unitaires et scénarios de bout en bout (SQLite)
SOCLE_TEST_POSTGRES_URL=postgres://user:pass@localhost:5432/socle   cargo test --manifest-path socle/Cargo.toml -p socle --test engine --test api   # mêmes scénarios sur PostgreSQL
make socle-validate             # socle.yaml : schéma strict et références croisées
node --check socle/crates/socle/assets/app.js
```

## Infrastructure et configuration

```bash
make cloud-validate             # terraform fmt / init -backend=false / validate
make checkov                    # IaC
make ansible-syntax             # tous les playbooks
make gitops-render              # rendu Kustomize de chaque overlay
make sre-check                  # règles Prometheus (SRE + socle)
```

## Application

```bash
(cd apps/erp && npm ci --ignore-scripts && npm run lint && npm test)
```

## Audit de conformité des serveurs

```bash
make security-audit             # CIS + durcissement en mode contrôle : écart = tâche « changed »
```

> Sous Windows, utiliser Git Bash, WSL ou exécuter les commandes depuis un environnement Linux.
