#!/usr/bin/env bash
# managed_backup.sh — sauvegarde quotidienne avec rotation

set -euo pipefail
umask 077  # les archives peuvent contenir /etc : lecture root uniquement

# Configuration
BACKUP_DIR="/var/backups"
LOG_DIR="/var/log/backups"
RETENTION=7

if [[ $EUID -ne 0 ]]; then
    echo "Exécuter en root/sudo (accès à /etc et $BACKUP_DIR requis)"
    exit 1
fi

# Création des répertoires si nécessaire
mkdir -p "$BACKUP_DIR" "$LOG_DIR"
chmod 700 "$BACKUP_DIR" "$LOG_DIR"

# Sauvegarde des fichiers importants
backup_files=(
    "/etc"
    "/var/www"
    "/home"
)

for file in "${backup_files[@]}"; do
    [[ -d "$file" ]] || continue
    tar --exclude='*.socket' -czf "$BACKUP_DIR/$(basename "$file")-$(date +%Y%m%d).tar.gz" "$file"
done

# Rotation : supprime les archives et logs plus anciens que RETENTION jours
find "$LOG_DIR" -type f -name "*.log" -mtime "+${RETENTION}" -delete
find "$BACKUP_DIR" -type f -name "*.tar.gz" -mtime "+${RETENTION}" -delete

# Journalisation
echo "Backup rotation completed at $(date)" >> "$LOG_DIR/backup_rotation.log"
