#!/usr/bin/env bash
# Evidence collection script for Linux endpoints
set -euo pipefail

CASE_ID="${1:-case-unknown}"
OUTPUT_DIR="/tmp/dfir-${CASE_ID}-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUTPUT_DIR"

echo "[*] Collecting evidence to $OUTPUT_DIR"

# System info
uname -a > "$OUTPUT_DIR/system_info.txt" 2>/dev/null
cat /etc/os-release >> "$OUTPUT_DIR/system_info.txt" 2>/dev/null
uptime >> "$OUTPUT_DIR/system_info.txt" 2>/dev/null

# Running processes
ps auxww > "$OUTPUT_DIR/processes.txt" 2>/dev/null

# Network connections
ss -tunap > "$OUTPUT_DIR/network_connections.txt" 2>/dev/null
netstat -rn > "$OUTPUT_DIR/routing.txt" 2>/dev/null

# Open files
lsof > "$OUTPUT_DIR/open_files.txt" 2>/dev/null

# Authentication logs
cp /var/log/auth.log "$OUTPUT_DIR/auth.log" 2>/dev/null || true
cp /var/log/secure "$OUTPUT_DIR/secure.log" 2>/dev/null || true
journalctl -u ssh >> "$OUTPUT_DIR/journal_ssh.txt" 2>/dev/null || true

# Users and groups
getent passwd > "$OUTPUT_DIR/passwd.txt" 2>/dev/null
getent group > "$OUTPUT_DIR/group.txt" 2>/dev/null
last -a > "$OUTPUT_DIR/last.txt" 2>/dev/null

# Scheduled tasks
cp /etc/crontab "$OUTPUT_DIR/crontab.txt" 2>/dev/null || true
cp -r /etc/cron.d "$OUTPUT_DIR/cron.d" 2>/dev/null || true

# List installed packages
if command -v dpkg >/dev/null 2>&1; then
    dpkg -l > "$OUTPUT_DIR/packages.txt" 2>/dev/null
elif command -v rpm >/dev/null 2>&1; then
    rpm -qa > "$OUTPUT_DIR/packages.txt" 2>/dev/null
fi

# File integrity baseline
find /etc /bin /sbin /usr/bin /usr/sbin -type f -printf '%T@ %p %m\n' 2>/dev/null | sort -k2 > "$OUTPUT_DIR/file_baseline.txt"

# Compress
TARBALL="${OUTPUT_DIR}.tar.gz"
tar -czf "$TARBALL" -C /tmp "$(basename "$OUTPUT_DIR")"
echo "[+] Evidence tarball: $TARBALL"
