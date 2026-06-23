# Backstage — Developer Portal

Configuration et templates Backstage pour le lab.

## Templates

- `templates/new-microservice/template.yaml` — Crée un microservice complet.
- `templates/new-microservice/catalog-info.yaml` — Enregistre le service dans le catalogue.

## Démarrage

```bash
# Backstage local
npx @backstage/create-app@latest backstage-app
yarn dev

# Enregistrer les templates
# Backstage UI → Create → Register Existing Component → URL du template.yaml
```
