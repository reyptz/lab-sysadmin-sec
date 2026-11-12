# OSSEC HIDS

Configuration d'agent OSSEC.

## Fichiers

- `ossec.conf` — Configuration d'agent OSSEC (journaux Debian et RHEL).

## Déploiement

```bash
# Linux (agent OSSEC déjà installé)
sudo cp security/ossec/ossec.conf /var/ossec/etc/
sudo systemctl restart ossec
```
