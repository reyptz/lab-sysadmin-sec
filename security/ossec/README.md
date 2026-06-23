# OSSEC HIDS

Configuration et scripts de déploiement pour OSSEC.

## Fichiers

- `ossec.conf` — Configuration d'agent OSSEC.
- `windows_hardening.ps1` — Script de durcissement Windows Server.

## Déploiement

```bash
# Linux
sudo apt-get install ossec-hids-agent
sudo cp security/ossec/ossec.conf /var/ossec/etc/
sudo systemctl restart ossec

# Windows (PowerShell admin)
.\security\ossec\windows_hardening.ps1
```
