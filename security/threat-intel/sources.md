# Threat Intelligence Sources

## Open Sources

- [MISP](https://www.misp-project.org/) — Plateforme de partage de threat intelligence.
- [AlienVault OTX](https://otx.alienvault.com/) — IOCs communautaires.
- [Abuse.ch](https://abuse.ch/) — MalwareBazaar, Feodo Tracker, ThreatFox.
- [URLhaus](https://urlhaus.abuse.ch/) — Base de malware URLs.
- [TheHive/Cortex](https://thehive-project.org/) — SOAR + analyse.

## Feeds recommandés

| Feed | Type | Usage |
|---|---|---|
| Emerging Threats | IDS rules | Suricata/Snort |
| Tor exit nodes | IPs | Blocage réseau |
| Blocklist.de | IPs | Fail2ban/Wazuh |
| CERT-FR | Alertes | Awareness |

## Intégration Wazuh

1. Télécharger les IOCs.
2. Convertir en listes CDB (`/var/ossec/etc/lists/`).
3. Créer des règles Wazuh utilisant `list` lookup.
