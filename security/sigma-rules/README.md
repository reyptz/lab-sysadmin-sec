# Sigma Rules

Règles de détection génériques compatibles avec le format Sigma.

## Règles disponibles

- `linux_auth_bruteforce.yml` — Détection de brute-force SSH sur Linux.
- `windows_privilege_escalation.yml` — Détection d'escalade de privilèges Windows.
- `lateral_movement.yml` — Détection de mouvement latéral réseau.

## Prérequis

```bash
pip install sigma-cli
```

## Conversion

```bash
# Pour Wazuh / OSSEC
sigma convert -t wazuh linux_auth_bruteforce.yml

# Pour Splunk
sigma convert -t splunk windows_privilege_escalation.yml
```
