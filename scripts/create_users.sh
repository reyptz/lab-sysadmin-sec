#!/bin/bash
# RHCSA — User/Group creation script
# Usage: sudo ./create_users.sh [users.txt]
# Format users.txt: username:group:role (role = user|admin)

set -euo pipefail

USER_FILE="${1:-/root/users.txt}"
LOG_FILE="/var/log/create_users.log"
# Les mots de passe initiaux sont générés aléatoirement par utilisateur et
# consignés dans un fichier lisible uniquement par root (à distribuer hors bande).

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') $1" | tee -a "$LOG_FILE"
}

if [[ $EUID -ne 0 ]]; then
   echo "Exécuter en root/sudo"
   exit 1
fi

if [[ ! -f $USER_FILE ]]; then
   echo "Fichier $USER_FILE non trouvé !"
   exit 1
fi

# Refuser un fichier modifiable par un utilisateur non privilégié.
if [[ $(stat -c '%u' "$USER_FILE") != 0 ]] || (( (8#$(stat -c '%a' "$USER_FILE") & 8#022) != 0 )); then
    echo "Le fichier utilisateurs doit appartenir à root et être non modifiable par le groupe/autres."
    exit 1
fi
command -v openssl >/dev/null

# Valider toutes les lignes avant de créer le moindre compte.
while IFS=':' read -r user group role extra || [[ -n "$user" ]]; do
    [[ -z "$user" || "$user" =~ ^# ]] && continue
    if [[ ! "$user" =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] ||
       [[ -n "$group" && ! "$group" =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] ||
       [[ "${role:-user}" != user && "$role" != admin ]] || [[ -n "$extra" ]]; then
        echo "Entrée utilisateur invalide ; aucun compte créé."
        exit 1
    fi
done < "$USER_FILE"

# Fichier exclusif : aucun écrasement ni suivi d'un lien symbolique préexistant.
umask 077
CREDENTIALS_FILE="$(mktemp /root/create_users_credentials.XXXXXX)"

while IFS=':' read -r user group role extra || [[ -n "$user" ]]; do
    # Skip comments and empty lines
    [[ -z "$user" || "$user" =~ ^# ]] && continue

    # Input validation
    if [[ ! "$user" =~ ^[a-z_][a-z0-9_-]*$ ]]; then
        log "ERROR: nom invalide '$user'"
        continue
    fi

    # Create group if it doesn't exist
    if [[ -n "$group" ]] && ! getent group "$group" &>/dev/null; then
        groupadd "$group"
        log "Groupe créé : $group"
    fi

    # Create user
    if id "$user" &>/dev/null; then
        log "Utilisateur $user existe déjà"
    else
        if [[ -n "$group" ]]; then
            useradd -m -s /bin/bash -g "$group" "$user"
        else
            useradd -m -s /bin/bash "$user"
        fi
        # Mot de passe initial unique et aléatoire (jamais prévisible).
        # Il est stocké hors bande dans $CREDENTIALS_FILE (0600, root only).
        initial_pass="$(openssl rand -base64 18)"
        echo "$user:${initial_pass}" | chpasswd
        echo "$user:${initial_pass}" >> "$CREDENTIALS_FILE"
        unset initial_pass
        chage -d 0 "$user"  # Force password change on first login
        chmod 700 "/home/$user"
        log "Créé : $user (mdp initial dans $CREDENTIALS_FILE, changement forcé au 1er login)"
    fi

    # Configure sudo for admin role
    if [[ "${role:-user}" == "admin" ]]; then
        if ! id -nG "$user" | grep -qw wheel; then
            usermod -aG wheel "$user" 2>/dev/null || usermod -aG sudo "$user"
            log "$user ajouté au groupe wheel/sudo"
        fi
    fi
done < "$USER_FILE"

log "Terminé !"