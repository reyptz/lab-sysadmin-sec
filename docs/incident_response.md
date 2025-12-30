# Security Operations Lab – Runbooks & Framework

**Date** : 27 décembre 2025  
**Auteur** : [Ton Nom]  
**Objectif** : Documenter les processus de détection, réponse, analyse post-incident et cartographie des menaces dans un laboratoire SysAdmin/Sécurité entreprise.

---

## 1. Cyber Threat Intelligence – Modèle STIX 2.1 / TAXII 2.1

### 1.1 Objets STIX Domain Objects (SDO) utilisés

| Objet STIX         | Description                                       | Exemple dans le Lab                                      |
|---------------------|--------------------------------------------------|----------------------------------------------------------|
| Indicator           | Pattern détectable (STIX pattern, Sigma, YARA)   | `[ipv4-addr:value = '192.168.99.10']`                    |
| Malware             | Logiciel malveillant simulé ou analysé           | RAT personnalisé, ransomware de test                     |
| Attack-Pattern      | Technique MITRE ATT&CK                           | T1110 – Brute Force                                      |
| Campaign            | Regroupement d’activités malveillantes           | "Campagne Brute Force SSH – Décembre 2025"               |
| Threat-Actor        | Entité attaquante                                | "RedTeam-Lab" ou "Script-Kiddie Simulé"                  |
| Identity            | Organisation victime                             | "LabCorp – Infrastructure de Test"                       |
| Sighting            | Observation réelle d’un IOC                      | Alerte Wazuh confirmant une IP malveillante              |

### 1.2 Cyber-observables (SCO)

- `ipv4-addr` → IP source d’attaque (ex: 192.168.99.10 – Kali)
- `domain-name` → Domaine C2 simulé
- `file` → Hashs SHA256 de binaires malveillants
- `user-account` → Comptes ciblés (root, admin, service accounts)
- `network-traffic` → Trafic suspect (port 22/TCP, HTTP vers evil.com)

### 1.3 Collections TAXII recommandées

| Collection                   | Contenu                                           | Accès            |
|------------------------------|---------------------------------------------------|------------------|
| lab-internal-incidents       | IOCs issus de nos incidents/ exercices            | Privé            |
| osint-ssh-bruteforce         | Blocklists publiques SSH (AbuseIPDB, etc.)        | Lecture seule    |
| mitre-attack-enterprise      | Matrice MITRE ATT&CK officielle                   | Lecture seule    |
| lab-redteam-iocs             | IOCs générés lors des exercices Red Team          | Interne + partage|

### 1.4 Exemple de Bundle STIX 2.1

```json
{
  "type": "bundle",
  "id": "bundle--f3e3c9d0-7d4a-4e5f-9b0d-1a2b3c4d5e6f",
  "objects": [
    {
      "type": "indicator",
      "spec_version": "2.1",
      "id": "indicator--8e2e1d2a-3b4c-5d6e-7f8g-9h0i1j2k3l4m",
      "created": "2025-12-27T10:00:00Z",
      "name": "Suspicious SSH Brute Force Source",
      "pattern": "[ipv4-addr:value = '192.168.99.10']",
      "pattern_type": "stix",
      "valid_from": "2025-12-27T10:00:00Z",
      "labels": ["malicious-activity", "brute-force"]
    }
  ]
}
```

## 2. Procédure de Réponse à Incident (NIST-based)

### Cycle : Détection → Analyse → Contention → Éradication → Récupération → Leçons apprises

#### 2.1 Détection
- Alerte Wazuh (severity ≥ 10)
- Anomalies Grafana/Prometheus
- Logs centralisés Syslog
- IDS Suricata (OPNsense)

#### 2.2 Analyse & Triage – Exemple concret
**Alerte** : Brute force SSH sur bastion (192.168.20.30)  
- IP source : 192.168.99.10 (Kali – exercice contrôlé)  
- 47 tentatives en 2 minutes  
- Corrélation : Règle Wazuh 100001 + Fail2Ban + OPNsense  
**MITRE** : T1110 – Brute Force (Credential Access)

#### 2.3 Contention (immédiate)
- Bannissement IP automatique (Fail2Ban + OPNsense)
- Termination sessions suspectes
- Isolement VLAN si nécessaire

#### 2.4 Éradication
- Investigation forensique (pas de compromission réelle)
- Reset mot de passe si nécessaire
- Scan FIM / ClamAV / Trivy

#### 2.5 Récupération
- Suppression règles de blocage manuelles
- Retour à l’état nominal
- Tests fonctionnels

#### 2.6 Leçons apprises
→ Lancer un RETEX dans les 48h (voir section suivante)

## 3. Runbook Retour d’Expérience (RETEX / Post-Incident Review)

### Objectifs
- Chronologie factuelle
- Analyse des causes racines (sans blâme)
- Actions correctives mesurables

### Phase 1 – Préparation
- Export logs (Wazuh, OPNsense, Syslog, Auditd)
- Timeline brute (UTC)
- Captures d’écran alertes

### Phase 2 – Réunion RETEX (45-60 min)

#### 3.1 Timeline reconstruite (exemple)

| Horodatage (UTC)      | Événement                             | Source                  |
|-----------------------|---------------------------------------|-------------------------|
| 2025-12-27 14:00      | Scan de ports (Nmap)                  | Suricata / OPNsense     |
| 2025-12-27 14:05      | Début brute force SSH (47 tentatives) | auth.log + Wazuh        |
| 2025-12-27 14:06      | Bannissement IP automatique           | Fail2Ban + OPNsense     |

#### 3.2 Analyse des causes – 5 Pourquoi (exemple)
1. Pourquoi le brute force a été tenté ? → Mot de passe faible sur compte test  
2. Pourquoi ? → Compte non couvert par playbook Ansible  
3. Pourquoi ? → Playbook incomplet pour comptes de service  
4. Pourquoi ? → Priorités focalisées sur serveurs critiques  
5. Pourquoi ? → Ressources limitées

#### 3.3 Points forts / Points faibles
- **+** Détection < 30s + blocage multi-couche  
- **–** Pas de notification Slack/Teams en temps réel

### Phase 3 – Plan d’action

| Action                                       | Priorité | Responsable | Deadline | Ticket         |
|----------------------------------------------|----------|-------------|----------|----------------|
| Étendre hardening Ansible aux comptes service| P0       | SysAdmin    | J+2      | LAB-SEC-045    |
| Webhook Wazuh → Slack (alertes ≥10)          | P1       | SecOps      | J+5      | LAB-SEC-046    |
| Revue règles Fail2Ban tous bastions          | P1       | SysAdmin    | J+7      | LAB-SEC-047    |

**Rapport final** : Archiver sous `docs/reports/YYYYMMDD_TypeIncident_RETEX.md`

---

## 4. Matrice MITRE ATT&CK – Couverture du Lab (v18 – 2025)

### 4.1 Détection

| Tactique           | Technique | Nom                        | Source de log           | Règle de détection               | Confiance |
|--------------------|-----------|----------------------------|-------------------------|----------------------------------|-----------|
| Initial Access     | T1078     | Valid Accounts             | auth.log / Event Log    | Wazuh : login suspect            | Moyen     |
| Credential Access  | T1110     | Brute Force                | auth.log                | >5 échecs + fréquence            | Haut      |
| Discovery          | T1046     | Network Service Scanning   | OPNsense / Suricata     | ET SCAN rules                    | Haut      |
| Defense Evasion    | T1070     | Indicator Removal          | bash_history / Audit    | Wazuh : suppression history      | Haut      |
| Command and Control| T1071     | Application Layer Protocol | Proxy / DNS logs        | Suricata : domaines suspects     | Moyen     |

### 4.2 Mitigation / Prévention

| Tactique              | Technique | Mesure de mitigation                        | Outil                     |
|-----------------------|-----------|---------------------------------------------|---------------------------|
| Credential Access     | T1110     | Bannissement IP après 3-5 échecs            | Fail2Ban + OPNsense       |
| Lateral Movement      | T1021     | Bastion obligatoire + VLAN + clé SSH only   | OPNsense + SSH config     |
| Persistence           | T1053     | Surveillance intégrité cron/jobs            | Wazuh FIM                 |
| Exfiltration          | T1048     | Filtrage egress strict                      | OPNsense firewall         |

### Légende
- 🟩 Bloqué automatiquement  
- 🟨 Détecté + alerte  
- 🟥 Gap à combler