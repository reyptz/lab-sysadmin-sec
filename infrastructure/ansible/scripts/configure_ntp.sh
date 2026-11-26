#!/usr/bin/env bash
# Installation et configuration NTP serveur/client sur Debian 12+

set -euo pipefail

# Serveur NTP interne surchargeable : INTERNAL_NTP_SERVER=ntp.example.com ./configure_ntp.sh
# Laisser vide pour n'utiliser que les pools publics.
INTERNAL_NTP_SERVER="${INTERNAL_NTP_SERVER:-ntp.monlab.local}"
NTP_CONF="/etc/ntpsec/ntp.conf"

if [[ $EUID -ne 0 ]]; then
    echo "Exécuter en root/sudo"
    exit 1
fi

# Mettre à jour les dépôts
apt update

# Installer ntpsec
apt install -y ntpsec

# Sauvegarde de l'ancienne conf
cp "$NTP_CONF" "${NTP_CONF}.bak"

# Écrire une conf de base (serveur interne + pools publics)
{
    echo "# Configuration générée par configure_ntp.sh"
    if [[ -n "$INTERNAL_NTP_SERVER" ]]; then
        echo "# Serveur NTP interne"
        echo "server ${INTERNAL_NTP_SERVER} iburst"
        echo ""
    fi
    echo "# Pools publics"
    echo "pool 0.fr.pool.ntp.org iburst"
    echo "pool 1.fr.pool.ntp.org iburst"
    echo "pool 2.fr.pool.ntp.org iburst"
    echo "pool 3.fr.pool.ntp.org iburst"
    # Restreindre les requêtes : serveur de temps uniquement pour le réseau local.
    echo "restrict default kod nomodify nopeer noquery limited"
    echo "restrict 127.0.0.1"
    echo "restrict ::1"
} > "$NTP_CONF"

# Redémarrer le service
systemctl restart ntpsec
systemctl enable ntpsec

echo "NTPsec configuré avec succès"
