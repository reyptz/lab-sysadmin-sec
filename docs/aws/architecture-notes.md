# Notes d'architecture AWS

Ces notes vivaient dans des `null_resource` Terraform qui ne créaient aucune
ressource. Elles relèvent de la documentation, pas de l'infrastructure.

## Well-Architected Framework (6 piliers)

1. Excellence opérationnelle
2. Sécurité
3. Fiabilité
4. Efficacité des performances
5. Optimisation des coûts
6. Durabilité

Processus : déclarer le workload dans AWS Well-Architected Tool, répondre aux
questions, traiter les risques élevés.

## Control Tower (si `organization_id` est fourni)

Activation manuelle depuis la console AWS :

- Landing Zone : région principale + région de reprise ;
- Account Factory : création standardisée des comptes Workloads / Security ;
- garde-fous préventifs (SCP) et détectifs (règles AWS Config).

## Optimisation des coûts

- Compute Savings Plans (1 an, paiement partiel anticipé, ~20–30 % sur EC2,
  Lambda et Fargate) ; analyse de couverture dans Cost Explorer.
- Instances réservées (RDS, ElastiCache) pour les charges stables.
- Instances Spot pour les traitements tolérants aux interruptions.
- S3 Intelligent-Tiering et règles de cycle de vie.
- Redimensionnement à partir de CloudWatch et Trusted Advisor.
- Arrêt nocturne des environnements de dev/test (EventBridge).

## Migration

- Stratégie 6R : réhéberger (MGN), replatformer (bases vers Aurora via DMS,
  `enable_dms`), refactoriser (monolithe vers microservices sur EKS),
  racheter (SaaS), conserver, retirer.
- Snow Family pour les transferts physiques : Snowcone (8 To, edge),
  Snowball Edge (80 To), Snowmobile (échelle exaoctet).
