# YARA Rules

Règles YARA pour la détection et l'analyse statique de malwares.

## Règles disponibles

- `generic_malware.yar` — Signatures génériques de comportements suspects.
- `suspicious_powershell.yar` — Détection de scripts PowerShell malveillants.

## Utilisation

```bash
yara security/yara-rules/generic_malware.yar /path/to/samples/
yara security/yara-rules/suspicious_powershell.yar /path/to/scripts/
```
