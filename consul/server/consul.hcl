datacenter = "lab-dc1"
data_dir   = "/opt/consul"

server           = true
bootstrap_expect = 1
ui_config {
  enabled = true
}

bind_addr    = "0.0.0.0"
client_addr  = "0.0.0.0"
advertise_addr = "{{ GetInterfaceIP \"eth0\" }}"

ports {
  http  = 8500
  https = 8501
  grpc  = 8502
}

encrypt = "uZ0...généré_avec_consul_keygen"  # Gossipping encryption

tls {
  defaults {
    ca_file   = "/etc/consul.d/tls/ca.crt"
    cert_file = "/etc/consul.d/tls/consul.crt"
    key_file  = "/etc/consul.d/tls/consul.key"
    verify_incoming = true
    verify_outgoing = true
  }
  internal_rpc {
    verify_server_hostname = true
  }
}

auto_encrypt {
  allow_tls = true
}

acl {
  enabled        = true
  default_policy = "deny"
  enable_token_persistence = true
}