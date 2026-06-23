# Chaos Engineering

Expériences de résilience pour l'API Facturation.

## Outils

- **Chaos Mesh** : pour Kubernetes (pod kill, CPU stress, network latency).
- **Litmus** : alternative Kubernetes-native.
- **Chaos Monkey** : pour instances cloud (GCE, EC2, Azure VM).
- **Toxiproxy** : pour simuler latence/réseau.

## Fichiers

- `experiments/pod-kill.yaml` — Tue un pod API invoices.
- `experiments/cpu-stress.yaml` — Stress CPU sur un pod.
- `experiments/network-latency.yaml` — Ajoute de la latence réseau vers la DB.
- `experiments/db-failure.yaml` — Simule une indisponibilité DB.

## Hypothèse de base

> "En cas de perte d'un pod, le circuit breaker et les retries doivent maintenir le SLO de 99,9 % sur 30 jours."

## Lancer une expérience

```bash
# Chaos Mesh
kubectl apply -f platform/chaos/experiments/pod-kill.yaml

# Toxiproxy (DB latency)
docker run -it --rm --network host shopify/toxiproxy-cli add db 0.0.0.0:5433 -u postgres:5432
```
