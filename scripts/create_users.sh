#!/bin/bash
# RHCSA — User/Group creation script
# Usage: sudo ./create_users.sh [users.txt]
# Format users.txt: username:group:role (role = user|admin)

set -euo pipefail

USER_FILE="${1:-/home/teti/users.txt}"
DEFAULT_PASS_SUFFIX="Ch@ngeM3!"
LOG_FILE="/var/log/create_users.log"

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

while IFS=':' read -r user group role extra; do
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
        echo "$user:${user}${DEFAULT_PASS_SUFFIX}" | chpasswd
        chage -d 0 "$user"  # Force password change on first login
        chmod 700 "/home/$user"
        log "Créé : $user (mdp doit être changé au premier login)"
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