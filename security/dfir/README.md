# Digital Forensics and Incident Response

Scripts et procédures pour la réponse à incident.

## Fichiers

- `collection.sh` — Collecte de preuves sur un endpoint Linux.
- `volatility_notes.md` — Notes d'analyse mémoire avec Volatility.

## Workflow

1. Isoler la machine (segmentation VLAN, firewall).
2. Collecter les preuves (RAM, disque, logs) via `collection.sh`.
3. Analyser la mémoire avec Volatility.
4. Corréler avec les logs SIEM.
