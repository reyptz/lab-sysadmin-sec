#!/usr/bin/env bash
# CIS Benchmarks audit script (Linux Level 1)
set -euo pipefail

REPORT_DIR="/var/log/cis-audit-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$REPORT_DIR"
REPORT="$REPORT_DIR/cis_report.txt"

echo "=== CIS Benchmarks Audit ===" > "$REPORT"
echo "Date: $(date)" >> "$REPORT"
echo "Host: $(hostname)" >> "$REPORT"
echo "" >> "$REPORT"

PASS=0
FAIL=0

check() {
    local id="$1"
    local description="$2"
    local command="$3"
    echo -n "[$id] $description ... " >> "$REPORT"
    if eval "$command" >> "$REPORT" 2>&1; then
        echo "PASS" >> "$REPORT"
        PASS=$((PASS + 1))
    else
        echo "FAIL" >> "$REPORT"
        FAIL=$((FAIL + 1))
    fi
    echo "" >> "$REPORT"
}

# 1.1.1.1 Disable mounting of cramfs
check "1.1.1.1" "Disable cramfs" "grep -q 'install cramfs /bin/true' /etc/modprobe.d/CIS.conf"

# 1.4.1 Ensure filesystem permissions are governed by systemd tmpfiles
check "1.4.1" "systemd tmpfiles" "test -d /etc/tmpfiles.d"

# 5.2.1 Ensure permissions on /etc/ssh/sshd_config are 644
check "5.2.1" "sshd_config permissions" "stat -c '%a' /etc/ssh/sshd_config | grep -q '^644$'"

# 5.2.4 Ensure SSH Protocol is 2
check "5.2.4" "SSH Protocol 2" "grep -q '^Protocol 2' /etc/ssh/sshd_config"

# 5.2.5 Ensure SSH LogLevel is appropriate
check "5.2.5" "SSH LogLevel" "grep -qE '^LogLevel (INFO|VERBOSE)' /etc/ssh/sshd_config"

# 5.2.12 Ensure SSH idle timeout is configured
check "5.2.12" "SSH idle timeout" "grep -qE '^ClientAliveInterval' /etc/ssh/sshd_config"

# 5.4.1 Ensure password creation requirements are configured
check "5.4.1" "Password requirements" "test -f /etc/security/pwquality.conf"

# 5.4.2 Ensure lockout for failed password attempts is configured
check "5.4.2" "Failed password lockout" "grep -q 'pam_faillock' /etc/pam.d/common-auth"

# 6.1.2 Ensure permissions on /etc/passwd are configured
check "6.1.2" "passwd permissions" "stat -c '%a' /etc/passwd | grep -q '^644$'"

# 6.2.1 Ensure password fields are not empty
check "6.2.1" "No empty passwords" "! awk -F: '($2==\"\"){print $1}' /etc/shadow"

echo "=== Summary ===" >> "$REPORT"
echo "PASS: $PASS" >> "$REPORT"
echo "FAIL: $FAIL" >> "$REPORT"

TARBALL="${REPORT_DIR}.tar.gz"
tar -czf "$TARBALL" -C /var/log "$(basename "$REPORT_DIR")"
echo "[+] Report saved: $TARBALL"
