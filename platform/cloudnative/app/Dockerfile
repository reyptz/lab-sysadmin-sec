# Utiliser une image de base officielle, spécifique et légère (Alpine)
FROM node:20-alpine

# Exécuter en mode production (désactive le code de debug, optimise Express)
ENV NODE_ENV=production

# Définir le répertoire de travail dans le conteneur
WORKDIR /usr/src/app

# Copier uniquement les fichiers de configuration de dépendances en premier
# Pourquoi ? Cela permet à Docker de mettre en cache la couche d'installation.
# Si tu modifies ton code (server.js) mais pas tes dépendances (package.json),
# Docker réutilisera le cache de cette étape, rendant le build instantané !
COPY package*.json ./

# Installer uniquement les dépendances de production.
# `npm ci` garantit un build reproductible à partir du package-lock.json
# (contrairement à `npm install` qui peut muter le lockfile).
RUN npm ci --omit=dev && npm cache clean --force

# Copier le reste du code de l'application en l'attribuant à l'utilisateur non-root
COPY --chown=node:node . .

# Sécurité SRE/DevOps : Ne jamais exécuter l'application en tant que 'root'
# L'image officielle Node.js fournit un utilisateur non privilégié nommé 'node'.
# C'est une bonne pratique de sécurité fondamentale (Principle of Least Privilege).
USER node

# Documenter le port que l'application va utiliser
EXPOSE 3000

# Vérification de santé native du conteneur (complète les probes Kubernetes)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD node -e "require('http').get('http://127.0.0.1:3000/health',r=>process.exit(r.statusCode===200?0:1)).on('error',()=>process.exit(1))"

# Commande pour démarrer l'application
CMD ["npm", "start"]
