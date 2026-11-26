output "aks_cluster_name" {
  description = "Cluster AKS (contexte kubeconfig : az aks get-credentials)"
  value       = azurerm_kubernetes_cluster.main.name
}

output "aks_oidc_issuer_url" {
  description = "Émetteur OIDC du cluster (Workload Identity de socle et des applications)"
  value       = azurerm_kubernetes_cluster.main.oidc_issuer_url
}

output "rhel_private_ips" {
  description = "Adresses privées des VMs RHEL (inventaire Ansible)"
  value       = azurerm_network_interface.rhel[*].private_ip_address
}
