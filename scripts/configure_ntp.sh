#!/bin/bash
# Installation et configuration NTP serveur/client sur Debian 12+

set -e  # quitte le script si une commande échoue

# Mettre à jour les dépôts
apt update

# Installer ntpsec
apt install -y ntpsec

# Exemple : config serveur NTP interne + pools publics
# ntpsec utilise /etc/ntpsec/ntp.conf
NTP_CONF="/etc/ntpsec/ntp.conf"

# Sauvegarde de l'ancienne conf
cp $NTP_CONF ${NTP_CONF}.bak

# Écrire une conf de base (serveur interne + pool publics)
cat > $NTP_CONF <<EOL
# Serveur NTP interne
server ntp.monlab.local iburst

# Pools publics
pool 0.fr.pool.ntp.org iburst
pool 1.fr.pool.ntp.org iburst
pool 2.fr.pool.ntp.org iburst
pool 3.fr.pool.ntp.org iburst
EOL

# Redémarrer le service
systemctl restart ntpsec
systemctl enable ntpsec

echo "NTPsec configuré avec succès"