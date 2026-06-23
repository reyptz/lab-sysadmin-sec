# Cloud Controls Matrix (CSA) — Continuous

## Domaines couverts

### IAM-03 — Identity and Access Management

- Moindre privilège appliqué via Ansible RBAC.
- MFA activé sur les comptes privilégiés.
- Révision trimestrielle des accès.

### CCC-01 — Change Control

- Versioning Git obligatoire.
- CI/CD traceable via GitHub Actions / GitLab CI.
- Approbation de pull requests avant merge.

### DSI-01 — Data Security & Information Lifecycle

- Chiffrement des données au repos (LVM crypt, Vault).
- Chiffrement TLS en transit (Nginx, certificats internes).
- Classification des données sensibles.

### IVS-01 — Infrastructure & Virtualization Security

- Hardening OS via CIS benchmarks.
- Segmentation VLAN.
- Bastion SSH pour l'administration.

### LOG-01 — Logging and Monitoring

- Centralisation Wazuh + rsyslog.
- Rétention 90 jours minimum.
- Alertes en temps réel.

### CCC-01 — Continuous Improvement

- Relecture mensuelle des règles Sigma / YARA.
- Mises à jour automatiques des agents de sécurité.
- Tabletop exercises trimestriels.
