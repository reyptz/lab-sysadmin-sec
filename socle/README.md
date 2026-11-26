# socle

Plan de contrôle du cycle de vie des déploiements et des infrastructures :
déploiement, CI/CD, supervision, scalabilité, environnements, sauvegardes et
restaurations, sécurité, incidents, maintenance — multi-cloud et on-premise,
Kubernetes et VMs RHEL.

- Architecture et choix : [`docs/socle/architecture.md`](../docs/socle/architecture.md)
- Exploitation : [`docs/socle/operations.md`](../docs/socle/operations.md)
- Spécification de la plateforme : [`socle.yaml`](../socle.yaml)

## Structure

| Dossier | Contenu |
|---|---|
| `crates/socle-core` | Noyau métier pur (aucune E/S) : modèle, validation, portes, RBAC, planificateurs, audit, incidents, sauvegardes, fenêtres |
| `crates/socle` | Adaptateurs et interfaces : stockage SQLite/PostgreSQL, exécuteur, notifications, OIDC, API REST, console, CLI |
| `crates/socle/assets` | Console web (HTML/CSS/JS sans dépendance, embarqués dans le binaire) |
| `deploy/kubernetes` | Déploiement HA de socle (Deployment, PDB, NetworkPolicy) |

## Développement

```bash
cargo test --workspace                                   # 64 tests (SQLite)
SOCLE_TEST_POSTGRES_URL=postgres://… cargo test -p socle --test engine --test api   # mêmes scénarios sur PostgreSQL
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p socle -- validate --spec ../socle.yaml
cargo run -p socle -- serve --bind 127.0.0.1:8080        # console : http://127.0.0.1:8080
```

Les tests d'intégration exécutent les scénarios de bout en bout sur une vraie
base : quatre yeux, portes de chaîne d'approvisionnement, retour arrière
automatique et incident, fenêtres et urgence, RPO et ordonnanceur, alertes,
maintenance, verrous, plans d'infrastructure, jetons et API HTTP.
