lab-sysadmin-sec/
└── firewall/
    ├── opnsense/
    │   ├── config.xml                 # Config complète OPNsense (exportée)
    │   ├── aliases.conf               # Aliases Firewall (bloqués, RFC1918, etc.)
    │   ├── rules_vlan10.conf          # Règles VLAN 10 (Users)
    │   ├── rules_vlan20.conf          # Règles VLAN 20 (Serveurs)
    │   ├── rules_vlan99.conf          # Règles VLAN 99 (SOC/Sécurité)
    │   ├── modsecurity_rules.conf     # Exemple règles ModSecurity custom
    │   ├── ospf_config.xml            # Extrait config OSPF
    │   └── iperf_setup.sh             # Script activation iperf pour tests perf
    └── pfsense/
        ├── config.xml                 # Config pfSense exportée
        ├── package_waf.conf           # Config Suricata + pfBlockerNG (WAF-like)
        ├── vpn_ipsec.conf             # Mobile IPSec + WireGuard
        └── vlan_segmentation.conf     # Règles segmentation + NAT