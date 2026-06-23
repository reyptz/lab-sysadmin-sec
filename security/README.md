# Sécurité — Blue Team / Red Team / GRC

Ce dossier regroupe les règles, scripts et procédures de sécurité du lab.

## Organisation

- `sigma-rules/` — Règles [Sigma](https://github.com/SigmaHQ/sigma) pour la détection de menaces.
- `yara-rules/` — Règles [YARA](https://virustotal.github.io/yara/) pour l'analyse statique de malwares.
- `ossec/` — Configuration OSSEC HIDS et hardening Windows.
- `dfir/` — Procédures et scripts de Digital Forensics and Incident Response.
- `redteam/` — Outils et méthodologies Red Team (OSINT, Python, procédures).
- `malware_analysis/` — Cadre d'analyse statique de malwares.
- `grc/` — Gouvernance, risque et conformité (risk assessment, CCM).
- `audit/` — Scripts d'audit de conformité (CIS Benchmarks).
- `threat-intel/` — Sources et indicateurs de threat intelligence.

## Utilisation rapide

```bash
# Audit CIS
sudo bash security/audit/cis_audit.sh

# Vérifier une règle YARA
yara security/yara-rules/generic_malware.yar /path/to/sample

# Convertir une règle Sigma
sigma convert -t splunk security/sigma-rules/linux_auth_bruteforce.yml
```

## Intégration Wazuh

Les règles Sigma et YARA peuvent être déclenchées via les commandes actives de Wazuh ou intégrées dans le pipeline SIEM.
