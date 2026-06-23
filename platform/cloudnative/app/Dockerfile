# Utiliser une image de base officielle, spécifique et légère (Alpine)
FROM node:20-alpine

# Définir le répertoire de travail dans le conteneur
WORKDIR /usr/src/app

# Copier uniquement les fichiers de configuration de dépendances en premier
# Pourquoi ? Cela permet à Docker de mettre en cache la couche d'installation (npm install).
# Si tu modifies ton code (server.js) mais pas tes dépendances (package.json),
# Docker réutilisera le cache de cette étape, rendant le build instantané !
COPY package*.json ./

# Installer uniquement les dépendances de production
RUN npm install --omit=dev

# Copier le reste du code de l'application
COPY . .

# Sécurité SRE/DevOps : Ne jamais exécuter l'application en tant que 'root'
# L'image officielle Node.js fournit un utilisateur non privilégié nommé 'node'.
# C'est une bonne pratique de sécurité fondamentale (Principle of Least Privilege).
USER node

# Documenter le port que l'application va utiliser
EXPOSE 3000

# Commande pour démarrer l'application
CMD ["npm", "start"]
