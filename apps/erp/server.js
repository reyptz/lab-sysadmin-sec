const express = require('express');
const helmet = require('helmet');
const { randomUUID } = require('node:crypto');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;
const instanceId = randomUUID();
const isProduction = process.env.NODE_ENV === 'production';

// Sécurité : en-têtes HTTP durcis (Helmet inclut déjà la suppression de X-Powered-By).
app.use(helmet({
  contentSecurityPolicy: {
    directives: {
      defaultSrc: ["'self'"],
      scriptSrc: ["'self'"],
      styleSrc: ["'self'", "'unsafe-inline'", 'https://fonts.googleapis.com'],
      fontSrc: ["'self'", 'https://fonts.gstatic.com'],
      imgSrc: ["'self'", 'data:'],
      connectSrc: ["'self'"],
      objectSrc: ["'none'"],
      frameAncestors: ["'none'"],
      // Le serveur local n'écoute pas en TLS ; HTTPS est terminé par l'ingress en production.
      upgradeInsecureRequests: isProduction ? [] : null
    }
  },
  strictTransportSecurity: isProduction,
  crossOriginEmbedderPolicy: false
}));

// Sécurité : ne pas divulguer le framework utilisé (fingerprinting).
app.disable('x-powered-by');

// Servir les fichiers statiques du dossier 'public'
app.use(express.static(path.join(__dirname, 'public')));

// Identifiant éphémère pour observer le load balancing sans exposer l'hôte.
app.get('/api/info', (req, res) => {
  res.set('Cache-Control', 'no-store');
  res.json({
    message: 'Meilleur Groupe Exposé',
    hostname: instanceId,
    version: '1.0.0',
    platform: 'application',
    uptime: Math.floor(process.uptime()),
    loadavg: []
  });
});

// Route de santé (Health Check)
// Kubernetes l'utilisera pour vérifier si notre application fonctionne correctement (Liveness/Readiness probes)
app.get('/health', (req, res) => {
  res.status(200).send('OK');
});

app.use((req, res) => {
  res.status(404).json({ error: 'Not found' });
});

// Express ne doit jamais renvoyer de stack trace ou de chemin local au client.
app.use((err, req, res, next) => {
  if (res.headersSent) return next(err);
  const status = err.status >= 400 && err.status < 500 ? err.status : 500;
  res.status(status).json({ error: status < 500 ? 'Invalid request' : 'Internal server error' });
});

module.exports = app;

if (require.main === module) {
  const server = app.listen(PORT, () => {
    console.log(`Server is running on port ${PORT}`);
  });
  server.headersTimeout = 10000;
  server.requestTimeout = 30000;
  server.timeout = 30000;
  server.keepAliveTimeout = 5000;
  server.maxRequestsPerSocket = 1000;
}
