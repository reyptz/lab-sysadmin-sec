# Guide d'exécution sur Docker Desktop

## 📋 Prérequis

- Docker Desktop installé et démarré
- Git ou accès au dossier du projet
- (Optionnel) Kubernetes activé dans Docker Desktop pour les déploiements K8s

## 🐳 Option 1 : Exécution simple avec Docker

### 1. Construire l'image Docker

```bash
# Naviguer dans le répertoire du projet
cd CloudNative

# Construire l'image
docker build -t cloudnative:latest .
```

**Vérification :**
```bash
docker images | grep cloudnative:latest
```

### 2. Lancer le conteneur

```bash
# Lancer le conteneur en mode détaché
docker run -d --name MNY -p 3000:3000 cloudnative:latest
```

### 3. Tester l'application

```bash
# Test de la route principale
curl http://localhost:3000

### 4. Consulter les logs

```bash
docker logs MNY

# Logs en temps réel
docker logs -f MNY
```

### 5. Arrêter et supprimer le conteneur

```bash
docker stop MNY
docker rm MNY
```

---

## ☸️ Option 2 : Exécution avec Kubernetes (Docker Desktop)

**Prérequis :** Kubernetes doit être activé dans Docker Desktop Settings → Kubernetes

### 1. Construire l'image (identique à Option 1)

```bash
cd CloudNative
docker build -t cloudnative:latest .
```

### 2. Déployer avec les manifests Kubernetes

```bash
# Appliquer le déploiement
kubectl apply -f ./k8s/app/deployment.yaml

# Appliquer le service
kubectl apply -f ./k8s/app/service.yaml
```

### 3. Vérifier le déploiement

```bash
# Vérifier l'état du déploiement
kubectl get deployments

# Vérifier les pods
kubectl get pods

# Vérifier les services
kubectl get services
```

### 4. Tester l'application

```bash
# Port-forward pour accéder au service
kubectl port-forward svc/cloud-native-demo-svc 3000:80

# Dans un autre terminal, tester
curl http://localhost:3000
```

### 5. Visualiser les logs

```bash
# Logs d'un pod spécifique
kubectl logs <POD_NAME>

# Logs en temps réel
kubectl logs -f <POD_NAME>
```

### 6. Nettoyer le déploiement Kubernetes

```bash
# Supprimer le déploiement et le service
kubectl delete -f ./k8s/app/deployment.yaml
kubectl delete -f ./k8s/app/service.yaml
```

---

## 🎯 Option 3 : Exécution avec Helm (Docker Desktop + K8s)

**Prérequis :** Helm installé sur la machine

### 1. Construire l'image

```bash
cd CloudNative
docker build -t cloud-native-demo:1.0.0 ./app
```

### 2. Installer le chart Helm

```bash
# Déployer avec Helm
helm install cloud-native ./helm/cloud-native-demo \
  --values ./helm/cloud-native-demo/values.yaml
```

### 3. Vérifier le déploiement Helm

```bash
# Lister les releases Helm
helm list

# Vérifier les ressources créées
kubectl get all -l app=cloud-native-demo
```

### 4. Tester l'application

```bash
# Port-forward
kubectl port-forward svc/cloud-native-demo-svc 3000:80

# Tester dans un autre terminal
curl http://localhost:3000
```

### 5. Supprimer le déploiement Helm

```bash
helm uninstall cloud-native
```

---

## 🔍 Commandes utiles

### Gestion des images Docker

```bash
# Lister toutes les images
docker images

# Supprimer une image
docker rmi cloud-native-demo:1.0.0

# Inspecter une image
docker inspect cloud-native-demo:1.0.0
```

### Gestion des conteneurs Docker

```bash
# Lister tous les conteneurs
docker ps -a

# Accéder au shell du conteneur en cours d'exécution
docker exec -it cloud-native-app /bin/sh

# Redémarrer un conteneur
docker restart cloud-native-app
```

### Gestion Kubernetes

```bash
# Décrire un déploiement
kubectl describe deployment cloud-native-demo

# Décrire un pod
kubectl describe pod <POD_NAME>

# Exécuter une commande dans un pod
kubectl exec -it <POD_NAME> -- /bin/sh

# Voir l'historique des déploiements
kubectl rollout history deployment/cloud-native-demo
```

### Débogage

```bash
# Vérifier si l'application répond
curl http://localhost:3000/health -v

# Vérifier les événements Kubernetes
kubectl get events

# Vérifier les ressources utilisées par les pods
kubectl top pods
```

---

## 📊 Architecture du projet

- **Application** : Node.js + Express (port 3000)
- **Routes disponibles** :
  - `GET /` : Retourne un message JSON avec le hostname
  - `GET /health` : Health check pour Kubernetes

---

## ⚠️ Dépannage

### Problème : Le conteneur ne démarre pas

```bash
# Vérifier les logs
docker logs cloud-native-app

# Vérifier les ressources disponibles
docker stats
```

### Problème : Port déjà utilisé

```bash
# Utiliser un port différent
docker run -d --name cloud-native-app -p 8080:3000 cloud-native-demo:1.0.0

# Ou tuer le conteneur précédent
docker stop cloud-native-app
docker rm cloud-native-app
```

### Problème : Image non trouvée en Kubernetes

```bash
# Assurez-vous que imagePullPolicy est "IfNotPresent" dans deployment.yaml
# Et que l'image est bien construite localement
docker images | grep cloud-native-demo
```