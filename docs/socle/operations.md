# socle — guide d'exploitation

## 1. Installation

### Poste de travail (lab, mode local)

```bash
make socle-build                       # binaire : socle/target/release/socle
socle validate                          # contrôle complet de socle.yaml
socle --local status                    # moteur embarqué, base .socle/socle.db
```

Le mode `--local` exécute exactement le même moteur sur une base SQLite locale.
Il convient au lab et à la CI. Les opérations soumises à approbation exigent le
serveur, car le demandeur ne peut pas s'approuver lui-même.

### Serveur (production, haute disponibilité)

1. **Amorçage de l'état Terraform** (une fois) : créer le bucket S3
   `lab-sysadmin-sec-tfstate`, versionné, chiffré KMS, accès public bloqué.
   Les fichiers `infrastructure/terraform/aws/backend/<env>.hcl` y pointent.
2. **Base PostgreSQL** : 14 ou plus, TLS imposé (`sslmode=verify-full`).
3. **Secrets Kubernetes** (via External Secrets Operator) dans le namespace
   `socle` : `socle-database` (clé `url`), `socle-git` (`id_ed25519`,
   `known_hosts` : clé de déploiement en écriture sur le dépôt), et en option
   `socle-notifications` (`SOCLE_SLACK_WEBHOOK`, `SOCLE_TEAMS_WEBHOOK`,
   `SOCLE_PAGERDUTY_KEY`).
4. **Identité cloud** par fédération, sans clé statique : IRSA (AWS),
   Workload Identity (Azure, GCP). Le compte de service `socle` porte les droits
   Terraform, Argo CD (lecture des Applications) et stockage de sauvegarde.
5. **Déploiement** : `kubectl apply -k socle/deploy/kubernetes` (2 réplicas,
   PDB, NetworkPolicy, conteneurs non-root en lecture seule).
6. **Premier administrateur** (accès direct à la base, une fois) :
   ```bash
   SOCLE_DATABASE_URL=postgres://… socle token create --bootstrap \
     --name amorcage --subject admin@exemple.fr --team platform --ttl 1d
   ```
   Configurez ensuite l'OIDC (`SOCLE_OIDC_ISSUER`, `SOCLE_OIDC_AUDIENCE`) ; les
   groupes du fournisseur d'identité sont mappés sur les équipes (`idp_groups`).
   Révoquez enfin le jeton d'amorçage.
7. **Jeton CI** : `socle token create --name ci --subject github:reyptz/lab-sysadmin-sec --team ci --ttl 90d`,
   à enregistrer dans le secret GitHub `SOCLE_CI_TOKEN`, avec `SOCLE_URL` en
   variable du dépôt.

### Variables du serveur

| Variable | Rôle |
|---|---|
| `SOCLE_DATABASE_URL` | `postgres://…` (HA) ou `sqlite:chemin?mode=rwc` |
| `SOCLE_SPEC`, `SOCLE_REPO_ROOT` | spécification et dépôt de travail |
| `SOCLE_BIND`, `SOCLE_WORKERS`, `SOCLE_PUBLIC_URL` | écoute, parallélisme, liens des notifications |
| `SOCLE_OIDC_ISSUER`, `SOCLE_OIDC_AUDIENCE`, `SOCLE_OIDC_GROUPS_CLAIM`, `SOCLE_OIDC_SUBJECT_CLAIM` | authentification SSO |
| `SOCLE_ALLOWED_PROGRAMS` | liste blanche des outils exécutables |
| `SOCLE_ENV_PASSTHROUGH` | préfixes de variables supplémentaires transmis aux outils |
| `SOCLE_LOG`, `SOCLE_LOG_FORMAT=json` | niveau et format des journaux |
| `SOCLE_NO_SCHEDULER` | désactive l'ordonnanceur sur un réplica |

## 2. Livraison applicative

```bash
# La CI enregistre la release (digest + preuves) et la déploie en dev.
socle release list --service erp-app

# Promotion du même artefact : staging puis production.
socle promote erp-app -e staging --wait
socle promote erp-app -e prod --ticket CHG-1234 --reason "Livraison 1.4.0" --dry-run   # aperçu : plan + portes
socle promote erp-app -e prod --ticket CHG-1234 --reason "Livraison 1.4.0"
socle approve <id-operation> --comment "RAS"                                         # par une autre personne (sre)
socle ops logs <id-operation> --follow
```

En production, le canary est conduit par Argo Rollouts (paliers 10, 25, 50,
75 et 100 %, avec analyse). socle attend ensuite la convergence, vérifie la
santé puis le taux d'erreur (PromQL). En cas d'échec, il rétablit
automatiquement la release précédente et ouvre un incident.

```bash
socle rollback erp-app -e prod --reason "régression facturation"   # pré-approuvé, hors fenêtre autorisé
socle scale erp-app -e prod --min 4 --max 12 --ticket CHG-1240     # bornes HPA, dans les limites déclarées
socle env                                                          # versions en place, dérives GitOps
```

**Procédure d'urgence** (`--emergency`, motif obligatoire) : elle contourne
les portes de processus (fenêtre, gel, ticket, incident) si la politique
l'autorise. Une seule approbation reste nécessaire, et l'opération figure
dans le rapport de conformité pour revue a posteriori. Les portes de sécurité
ne se contournent jamais.

## 3. Infrastructure multi-cloud

```bash
socle infra plan aws-core -e prod                         # calcule et empreinte le plan
socle ops show <id-plan>                                  # ressources modifiées
socle infra apply aws-core -e prod --plan-op <id-plan> --ticket CHG-1300
```

L'application recalcule le plan et le compare à l'empreinte approuvée. Si
l'infrastructure a changé entre-temps, elle refuse de s'exécuter. Une détection
de dérive (`drift_check: 24h`) lance des plans planifiés et notifie
`drift.detected`.

Stacks déclarées dans `socle.yaml` (même convention : `env/<env>.tfvars`
versionné sans secret, `backend/<env>.hcl` pour l'état distant verrouillé) :

| Stack | Dossier | Rôle | Environnements |
|---|---|---|---|
| `aws-core` | `infrastructure/terraform/aws` | Production principale (VPC, EKS, RHEL EC2, Aurora) | dev, staging, prod |
| `azure-dr` | `infrastructure/terraform/azure` | Reprise d'activité (AKS, VMs RHEL optionnelles) | prod |
| `gcp-preview` | `infrastructure/terraform/gcp` | GKE privé, environnements éphémères et analytique | dev |
| `ovh-backup` | `infrastructure/terraform/ovh` | Copies de sauvegarde souveraines, Managed Kubernetes (prod) | prod |
| `scaleway-backup` | `infrastructure/terraform/scaleway` | Seconde copie souveraine, Kapsule (prod) | prod |

Identifiants (jamais versionnés, fédérés de préférence) : `TF_VAR_subscription_id`
+ Workload Identity (Azure), `TF_VAR_project_id` + Workload Identity (GCP),
`TF_VAR_service_name` + `OVH_*` (OVHcloud), `SCW_*` (Scaleway). Les états
distants se créent une fois hors Terraform (compte de stockage Azure, bucket
GCS, buckets S3 OVHcloud/Scaleway). Ajouter un fournisseur = déclarer ses
cibles et ses stacks dans `socle.yaml` ; aucun code n'est nécessaire.

## 4. Sauvegardes, restaurations, exercices

```bash
socle backup status                                  # RPO tenu ? dernier exercice dans le RTO ?
socle backup run rhel-files -e prod
socle backup drill erp-k8s -e prod                   # restauration jetable, mesure du RTO
socle backup restore rhel-files -e prod --confirm prod --ticket CHG-1400              # à côté de l'existant
socle backup restore rhel-files -e prod --in-place --confirm prod --ticket CHG-1401   # écrase (approbation requise)
socle backup drill aurora-erp -e prod                # clone PITR Aurora mesuré, supprimé ensuite
```

**Aurora (`aurora-erp`)** : le RPO de la base repose sur le PITR natif. socle
le **vérifie** toutes les 15 minutes : l'action `check` de
`playbooks/aurora_pitr.yml` échoue si le dernier point restaurable a plus de
15 minutes, ce qui déclenche l'alerte et l'incident RPO. Chaque semaine,
l'exercice clone le cluster au dernier instant restaurable (copy-on-write),
crée une instance de test, contrôle la connectivité puis supprime tout ;
sa durée est comparée au RTO d'une heure. La rétention quotidienne reste
assurée par AWS Backup (35 jours). Prérequis : l'identité de socle peut
créer et supprimer des clusters et instances RDS `socle-drill-*` dans le
sous-réseau et les groupes de sécurité d'Aurora.

L'ordonnanceur lance les sauvegardes à la cadence déclarée (`every`) et les
exercices (`drill.every`). Il ouvre un incident Sev2 si le RPO est dépassé en
production, et le résout dès que le RPO est de nouveau tenu.

Prérequis sur les hôtes (provisionnés hors socle, jamais transmis par lui) :
`/etc/socle/restic.password` et `/etc/socle/restic.env` (root, 0600), ainsi
que la configuration pgBackRest (`/etc/pgbackrest/pgbackrest.conf`) pour les
politiques `postgres`. Les dépôts `copies` (OVHcloud, Scaleway) appliquent la
règle 3-2-1 chez un autre fournisseur. Activez le verrouillage d'objets (WORM)
sur les buckets de sauvegarde pour vous protéger des rançongiciels.

## 5. Incidents

```bash
socle incident open --title "API facturation en 5xx" --severity sev2 -e prod --service erp-app
socle incident ack <id> --note "pris en charge"
socle incident note <id> "rollback lancé"
socle incident resolve <id>
socle incident update <id> --root-cause "migration non rétrocompatible"
socle incident close <id>                    # Sev1/Sev2 : cause racine ou postmortem obligatoire
socle incident postmortem <id> > pm.md
```

Les alertes Alertmanager (`severity=critical|page`) ouvrent des incidents
dédupliqués et les résolvent automatiquement : voir
`observability/alertmanager.yml`. Un incident Sev2 ou plus grave ouvert
sur un environnement bloque les déploiements selon la politique (porte
« incidents »).

## 6. Maintenance et correctifs

```bash
socle maintenance start -e prod --duration 2h --reason "Montée de version EKS"   # gèle les déploiements, silence les alertes
socle maintenance end <id>
socle patch rhel-onprem -e prod --ticket CHG-1500     # dnf par lots, redémarrage si nécessaire, contrôle des services
socle runbook audit-cis-rhel -e prod                  # audit CIS en mode contrôle
```

## 7. Gouvernance

```bash
socle audit verify                       # intégrité de toute la chaîne
socle audit export --out audit.jsonl     # archivage (SIEM, WORM)
socle compliance --days 90 --markdown > preuves.md
socle token list                         # revue des jetons (expiration, dernier usage)
```

## 8. Dépannage

| Symptôme | Cause probable | Action |
|---|---|---|
| Opération refusée | une porte a échoué | `socle ops show <id>` liste chaque porte ; `--dry-run` pour tester avant soumission |
| « verrou occupé » | une autre opération modifie la même ressource | nouvel essai automatique toutes les 15 s |
| « spécification modifiée depuis la planification » | `socle.yaml` a changé (rechargement) | soumettre à nouveau l'opération |
| Opération en échec « perte du worker » | réplica arrêté pendant l'exécution | vérifier l'état réel puis relancer ; rien n'est rejoué automatiquement |
| « programme non autorisé » | outil hors liste blanche | ajuster `SOCLE_ALLOWED_PROGRAMS` |
| Dérive GitOps signalée | overlay modifié hors socle | redéployer la release voulue avec socle (ou revenir sur le commit manuel) |

## 9. Suites prévues

- Rendre bloquants les scans Checkov des zones d'accueil Azure, GCP, OVHcloud
  et Scaleway (aujourd'hui en rapport non bloquant) après traitement des écarts.
- Empaqueter l'agent eBPF Shinra (unité systemd sur hôtes dédiés).
- Contrôle applicatif (requête SQL) dans l'exercice Aurora, en plus de la connectivité.
