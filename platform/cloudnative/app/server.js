const express = require('express');
const os = require('os');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;

// Sécurité : ne pas divulguer le framework utilisé (fingerprinting).
app.disable('x-powered-by');

// Servir les fichiers statiques du dossier 'public'
app.use(express.static(path.join(__dirname, 'public')));

// API qui retourne les informations de l'instance courante (très utile pour le load balancing)
app.get('/api/info', (req, res) => {
  res.json({
    message: 'Meilleur Groupe Exposé',
    hostname: os.hostname(),
    version: '1.0.0',
    platform: os.platform(),
    uptime: Math.floor(os.uptime()),
    loadavg: os.loadavg()
  });
});

// Route de santé (Health Check)
// Kubernetes l'utilisera pour vérifier si notre application fonctionne correctement (Liveness/Readiness probes)
app.get('/health', (req, res) => {
  res.status(200).send('OK');
});

// Rediriger toutes les autres requêtes vers l'application frontend
app.get('*', (req, res) => {
  res.sendFile(path.join(__dirname, 'public', 'index.html'));
});

module.exports = app;

if (require.main === module) {
  app.listen(PORT, () => {
    console.log(`Server is running on port ${PORT}`);
  });
}
