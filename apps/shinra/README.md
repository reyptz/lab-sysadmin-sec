# Shinra Defense — plateforme de défense active

Plateforme de défense active : déploiement de leurres (honeypots), détection de
menaces par eBPF, extraction d'artefacts (clés AES/RSA, shellcodes) et réponse
aux incidents par playbooks. Projet de démonstration intégré au lab ; licence
AGPL v3 (voir [LICENSE](LICENSE)).

## Composants

| Dossier | Langage | Rôle |
|:---|:---|:---|
| [src-agent](src-agent/README.md) | Rust + C (eBPF) | Surveillance des syscalls (`openat`, `read`, `connect`), memory scraper, kill-switch, suivi des leurres |
| [src-engine](src-engine/README.md) | Python (FastAPI) | API REST + WebSocket, base SQLite, corrélation RAG (ChromaDB), gestion des conteneurs leurres (Docker/Podman) |
| [src-ui](src-ui/) | HTML/CSS | Dashboard SOC : vue d'ensemble, flux d'événements, nœuds eBPF, artefacts |
| [playbooks](playbooks/README.md) | YAML | Définitions de contre-mesures : réponse ransomware, provisionnement de leurres, extraction d'artefacts, isolation réseau, durcissement |
| [scripts](scripts/README.md) | Nix | Environnement de développement reproductible (`flake.nix`) |

## Prérequis

- Docker ou Podman (conteneurs leurres)
- Python 3.9+
- Rust 1.70+, clang/llvm et en-têtes libbpf (agent eBPF)
- Noyau Linux 5.5+ (eBPF)
- Nix (optionnel)

## Démarrage

Depuis ce dossier (`apps/shinra/`) :

```bash
# Moteur Python
cd src-engine
python -m venv venv && source venv/bin/activate
pip install -r requirements.txt
python main.py            # API sur http://127.0.0.1:8000

# Agent Rust (Linux, root requis pour eBPF)
cd ../src-agent
cargo build --release
sudo ./target/release/shinra-agent
```

## Déploiement (production)

Le moteur est livré en image immuable `ghcr.io/reyptz/lab-sysadmin-sec/shinra-engine`
(`src-engine/Dockerfile`, construite, testée, analysée et signée par le workflow
`shinra.yml`), puis déployé par **socle** sur les serveurs RHEL (service
`shinra-engine` de `socle.yaml`, Podman/Quadlet, un hôte témoin puis le reste) :

```bash
socle deploy shinra-engine -e prod --release <digest|version> --ticket CHG-1234 --dry-run
socle deploy shinra-engine -e prod --release <digest|version> --ticket CHG-1234
socle rollback shinra-engine -e prod        # retour à la release précédente
```

Données persistantes : `/var/lib/shinra` (base SQLite, ChromaDB, cache des
modèles), sauvegardées par la politique `rhel-files`. Le contrôle de santé
(`/health`) est fait par lot pendant le déploiement. La gestion des leurres
nécessite l'accès au socket Podman/Docker de l'hôte : à monter explicitement
(`volumes` dans `socle.yaml`) uniquement sur des hôtes dédiés.

L'agent eBPF (`src-agent`) reste un service système de l'hôte (CAP_BPF,
CAP_SYS_ADMIN), hors conteneur ; son empaquetage (unité systemd) est à venir.

## Sécurité

- L'agent eBPF exige `CAP_BPF` et `CAP_SYS_ADMIN` : ne l'exécuter que sur des
  hôtes dédiés.
- Les dumps mémoire sont analysés puis détruits ; aucune API externe n'est
  appelée (fonctionnement 100 % local).
- Ce composant est un prototype : l'agent et le moteur n'ont pas de suite de
  tests, et les playbooks de `playbooks/` sont des définitions déclaratives
  qu'aucun exécuteur n'interprète encore.
