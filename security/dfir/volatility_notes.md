# Volatility Memory Analysis Notes

## Acquisition

```bash
# Linux
sudo apt install lime-dkms
sudo insmod /lib/modules/$(uname -r)/kernel/drivers/lime.ko "path=/tmp/mem.lime format=lime"

# Windows
winpmem_mini_x64.exe -o mem.dmp
```

## Analysis with Volatility 3

```bash
# Info
volatility -f mem.dmp windows.info

# Processes
volatility -f mem.dmp windows.pslist
volatility -f mem.dmp windows.pstree
volatility -f mem.dmp windows.psxview

# Network
volatility -f mem.dmp windows.netscan

# DLLs / injected code
volatility -f mem.dmp windows.dlllist
volatility -f mem.dmp windows.malfind

# Registry
volatility -f mem.dmp windows.registry.printkey --key "Software\Microsoft\Windows\CurrentVersion\Run"

# Files
volatility -f mem.dmp windows.filescan
volatility -f mem.dmp windows.dumpfiles --physaddr 0x... -o /tmp/dumped
```

## Linux with Volatility 3

```bash
volatility -f mem.lime linux.pslist
volatility -f mem.lime linux.bash
volatility -f mem.lime linux.proc.Maps
volatility -f mem.lime linux.netstat
```

## Indicators

- Processus sans image parent (PPID 0 ou inexistant).
- Sections mémoire exécutables avec RWX.
- Connexions réseau depuis des processus systèmes inhabituels.
- Hooks SSDT / callbacks suspects.
