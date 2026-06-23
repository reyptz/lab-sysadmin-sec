# GCP Chaos Engineering & Postmortem Culture

Ce document couvre les pratiques **Chaos Engineering** et la culture **Blameless Postmortem** pour le parcours **Google Cloud Professional Cloud DevOps Engineer**.

## 1. Chaos Engineering

### Principes

- **Hypothesis-driven** : partir d'une hypothese sur le comportement attendu du systeme.
- **Production-like** : tester dans un environnement representatif (staging ou prod avec blast radius controle).
- **Blast radius minimal** : commencer petit, mesurer, arreter immediatement si necessaire.
- **Automatise** : les experimentations doivent etre reproductibles et planifiees.

### Outils GCP

| Outil | Usage |
|:---|:---|
| **Chaos Monkey** (Netflix) | Terminer aleatoirement des instances GCE/VM |
| **Gremlin** (SaaS) | Chaos engineering managed pour GKE, Cloud Run, GCE |
| **Litmus** (open source) | Chaos experiments sur Kubernetes (GKE) |
| **Chaos Mesh** (open source) | Chaos experiments avancees sur Kubernetes |
| **Cloud Build / Cloud Scheduler** | Orchestrer des experimentations automatisees |
| **Cloud Monitoring + Cloud Logging** | Mesurer l'impact et collecter les metriques |

### Experiences type pour le lab

| ID | Experience | Systeme cible | Metriques de succes |
|:---|:---|:---|:---|
| CH-01 | Terminer 1 pod GKE | GKE autopilot | Redemarrage automatique, SLO respecte |
| CH-02 | Saturer CPU d'une VM | GCE app | Autoscaling ou alerte, pas de panne visible |
| CH-03 | Induire une latence reseau | Cloud Run | Circuit breaker / retry, erreurs < 1% |
| CH-04 | Remplir le disque d'une VM | GCE app | Alertes, rotation logs, pas de crash |
| CH-05 | Simuler indisponibilite GCS | Backup bucket | Fallback vers replica, alerte ops |

### Exemple Litmus experiment (GKE)

```yaml
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: gke-pod-delete
spec:
  appinfo:
    appns: default
    applabel: "app=lab-app"
    appkind: deployment
  chaosServiceAccount: litmus-admin
  experiments:
    - name: pod-delete
      spec:
        components:
          env:
            - name: TOTAL_CHAOS_DURATION
              value: "30"
            - name: CHAOS_INTERVAL
              value: "10"
            - name: FORCE
              value: "false"
```

### Calendrier de chaos

- **Game Day** mensuel : 2h d'experimentation en equipe.
- **Automatisation** : 1 experiment par sprint via Cloud Build schedule.
- **Runbook** : chaque experiment a un runbook de rollback et d'observation.

## 2. Postmortem Culture

### Principes du blameless postmortem

1. **Pas de blame** : concentrer l'analyse sur le systeme, pas sur les individus.
2. **Facts over narratives** : s'appuyer sur les logs, metriques, traces.
3. **Action items concrets** : chaque postmortem genere des tickets priorises.
4. **Partage transparent** : diffusion a toute l'equipe et dans la knowledge base.
5. **Review periodique** : relecture mensuelle des postmortems pour detecter les patterns.

### Template de postmortem

```markdown
# Postmortem : [Incident ID] - [Titre]

## Résumé (TL;DR)
- Date / heure :
- Duree :
- Impact :
- Resolution :

## Timeline (UTC)
- HH:MM : Detection
- HH:MM : Escalade
- HH:MM : Mitigation
- HH:MM : Resolution

## Cause racine
[Description detaillee]

## Detection
- Alerte / Monitoring :
- Client / SRE :

## Remediation immediate
1. ...
2. ...

## Action items
| ID | Action | Owner | Due | Priority |
|----|--------|-------|-----|----------|
| 1  | ...    | ...   | ... | ...      |

## Leçons apprises
- ...

## Annexes
- Lien vers les logs (Cloud Logging)
- Lien vers le dashboard (Cloud Monitoring)
- Lien vers l'alerte
```

## 3. Integration dans le lab

- Les runbooks `docs/incident_response.md` et `infrastructure/cloud/SRE.md` sont les points de depart.
- Les metriques SLO/error budget sont dans `infrastructure/cloud/SRE.md`.
- Les experimentations de chaos peuvent etre lancees via Cloud Build ou kubectl.
