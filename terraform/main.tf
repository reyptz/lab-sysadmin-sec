provider "proxmox" {
  pm_api_url      = "https://ton-proxmox:8006/api2/json"
  pm_user         = "root@pam"
  pm_password     = "tonpassword"
  pm_tls_insecure = true
}

resource "proxmox_vm_qemu" "debian_server" {
  name        = "debian-server"
  target_node = "pve"
  clone       = "debian-13-template"
  cores       = 2
  memory      = 2048
  ipconfig0   = "ip=192.168.19.130/24,gw=192.168.19.1"
}

resource "proxmox_vm_qemu" "ubuntu_server" {
  name        = "ubuntu-server"
  target_node = "pve"
  clone       = "ubuntu-24-template"
  cores       = 2
  memory      = 2048
  ipconfig0   = "ip=192.168.19.131/24,gw=192.168.19.1"
}

resource "proxmox_vm_qemu" "windows_server" {
  name        = "windows-server-2022"
  target_node = "pve"
  clone       = "windows-2022-template"
  cores       = 4
  memory      = 4096
  ipconfig0   = "192.168.19.132/24,gw=192.168.19.1"
  scsihw      = "virtio-scsi-pci"
  boot        = "cdn"
  ostype      = "win10"
}
