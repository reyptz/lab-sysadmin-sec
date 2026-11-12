# OPA Gatekeeper Policies

## Installation

```bash
kubectl apply -f https://raw.githubusercontent.com/open-policy-agent/gatekeeper/master/deploy/gatekeeper.yaml
make policies-gatekeeper
```

Les `ConstraintTemplate` doivent être appliqués et établis **avant** les
`Constraint` (sinon le CRD de la contrainte n'existe pas encore) ; la cible
`make policies-gatekeeper` respecte cet ordre.

## Policies

- `constraint-template-privileged.yaml` + `constraint-privileged.yaml` — Interdit les conteneurs privilégiés.
- `constraint-template-host-namespace.yaml` + `constraint-host-namespace.yaml` — Interdit `hostNetwork`, `hostPID`, `hostIPC`.

Les labels obligatoires et la restriction des registres d'images sont couverts
par les policies Kyverno (`../kyverno/`).
