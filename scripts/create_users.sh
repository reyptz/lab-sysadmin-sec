#!/bin/bash create_users.sh
# Script pour créer plusieurs utilisateurs à partir d'un fichier users.txt (un nom par ligne)
# Mot de passe par défaut : username123 (à changer en prod !)

USER_FILE="/home/teti/users.txt"
DEFAULT_PASS_SUFFIX="123"

if [[ $EUID -ne 0 ]]; then
   echo "Exécuter en root/sudo"
   exit 1
fi

if [[ ! -f $USER_FILE ]]; then
   echo "Fichier $USER_FILE non trouvé !"
   exit 1
fi

while IFS= read -r user; do
    if id "$user" &>/dev/null; then
        echo "Utilisateur $user existe déjà"
    else
        useradd -m -s /bin/bash "$user"
        echo "$user:${user}${DEFAULT_PASS_SUFFIX}" | chpasswd
        echo "Créé : $user avec mdp ${user}${DEFAULT_PASS_SUFFIX}"
    fi
done < "$USER_FILE"

echo "Terminé !"