#!/bin/bash managed_backup.sh

# Configuration
BACKUP_DIR="/var/backups"
LOG_DIR="/var/log/backups"
RETENTION=7

# Création des répertoires si nécessaire
mkdir -p "$BACKUP_DIR" "$LOG_DIR"

# Rotation des logs
find "$LOG_DIR" -type f -name "*.log" -mtime +7 -exec rm -f {} \;

# Rotation des sauvegardes
find "$BACKUP_DIR" -type f -name "*.tar.gz" -mtime +7 -exec rm -f {} \;

# Sauvegarde des fichiers importants
backup_files=(
    "/etc"
    "/var/www"
    "/home"
)

for file in "${backup_files[@]}"; do
    tar --exclude='*.socket' -czf "$BACKUP_DIR/$(basename "$file")-$(date +%Y%m%d).tar.gz" "$file"
done

# Nettoyage des anciennes sauvegardes
find "$BACKUP_DIR" -type f -name "*.tar.gz" -mtime +7 -exec rm -f {} \;

# Journalisation
echo "Backup rotation completed at $(date)" >> "$LOG_DIR/backup_rotation.log"
