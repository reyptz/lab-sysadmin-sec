# OPA Gatekeeper Policies

## Installation

```bash
kubectl apply -f https://raw.githubusercontent.com/open-policy-agent/gatekeeper/master/deploy/gatekeeper.yaml
kubectl apply -f platform/devsecops/policies/kubernetes/gatekeeper/
```

## Policies

- `deny-privileged.yaml` — Interdit les conteneurs privilégiés.
- `deny-host-namespace.yaml` — Interdit `hostNetwork`, `hostPID`, `hostIPC`.
- `required-labels.yaml` — Exige des labels obligatoires.
- `allowed-registries.yaml` — Restreint les registres d'images.
