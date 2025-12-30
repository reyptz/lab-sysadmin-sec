#!/bin/sh
# À exécuter sur OPNsense (via shell ou package iperf)
pkg install iperf3

# Lancer serveur iperf sur port 5201
iperf3 -s -p 5201 -D

# Règle firewall auto (à ajouter via GUI ou API)
# Allow TCP/UDP 5201 depuis VLANs autorisés