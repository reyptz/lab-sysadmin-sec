#!/bin/bash
set -e

export DEBIAN_FRONTEND=noninteractive

# Mise a jour
apt-get update
apt-get upgrade -y

# Packages RHCSA / hardening
apt-get install -y \
  fail2ban \
  ufw \
  chrony \
  unattended-upgrades \
  auditd \
  acl

# Hardening SSH
sed -i 's/^#*PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config
sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config
systemctl restart sshd

# Firewall minimal
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw --force enable

# Services
systemctl enable fail2ban chrony unattended-upgrades auditd
systemctl start fail2ban chrony unattended-upgrades auditd

# Agent Azure Monitor (VM extension gere par Terraform, mais on s'assure du port)
# Aucun secret local : l'identite Managed Identity est utilisee.
echo "Cloud-init termine"
