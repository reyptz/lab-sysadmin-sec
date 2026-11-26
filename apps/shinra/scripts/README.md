# Scripts Shinra Defense

- `flake.nix` — environnement de développement reproductible (Nix flakes) :
  Python, Rust, clang/llvm et en-têtes libbpf pour le moteur et l'agent eBPF.

```bash
nix develop ./apps/shinra/scripts
```

Les anciens scripts `deploy.sh`, `rollback.sh` et `health-check.sh` ont été
retirés : le déploiement, le retour arrière et les contrôles de santé du
moteur passent par socle (voir `../README.md`, section « Déploiement »).
