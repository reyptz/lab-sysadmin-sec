datacenter = "lab-dc1"
data_dir   = "/opt/consul"

retry_join = ["192.168.19.10"]  # IP du serveur Consul

bind_addr   = "0.0.0.0"
client_addr = "127.0.0.1"

ports {
  http = -1
  https = 8501
  grpc = -1
  grpc_tls = 8503
}

encrypt = "uZ0...même_clé"

tls {
  defaults {
    verify_incoming = true
    verify_outgoing = true
    ca_file         = "/etc/consul.d/tls/ca.crt"
  }
  internal_rpc {
    verify_server_hostname = true
  }
}

auto_encrypt {
  tls = true
}

acl {
  enabled = true
  default_policy = "deny"
  enable_token_persistence = true
}