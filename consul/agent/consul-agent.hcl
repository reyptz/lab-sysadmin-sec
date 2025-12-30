datacenter = "lab-dc1"
data_dir   = "/opt/consul"

retry_join = ["192.168.19.10"]  # IP du serveur Consul

bind_addr   = "0.0.0.0"
client_addr = "0.0.0.0"

encrypt = "uZ0...même_clé"

tls {
  defaults {
    verify_incoming = true
    verify_outgoing = true
    ca_file         = "/etc/consul.d/tls/ca.crt"
  }
}

auto_encrypt {
  tls = true
}