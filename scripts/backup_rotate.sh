#!/bin/bash backup_rotate.sh
# Backup quotidien avec rotation (7 jours)

LOG_DIR="/var/log/testapp"      # À adapter
BACKUP_DIR="/backup/logs"
RETENTION_DAYS=7
DATE=$(date +%Y-%m-%d)

mkdir -p "$BACKUP_DIR"

if [[ -d "$LOG_DIR" ]]; then
    tar -czf "$BACKUP_DIR/logs_$DATE.tar.gz" -C "$LOG_DIR" .
else
    echo "Dossier $LOG_DIR introuvable, backup ignoré"
fi

# Suppression anciens backups
find "$BACKUP_DIR" -type f -name "*.tar.gz" -mtime +$RETENTION_DAYS -delete

echo "Backup du $DATE créé et rotation effectuée"