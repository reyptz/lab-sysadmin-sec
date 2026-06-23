# Windows Server 2022 Hardening Script
# Role: AD DS, DNS, DHCP, GPO

# Kerberos max ticket age
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\Lsa\Kerberos\Parameters" -Name "MaxTicketAge" -Value 10 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\Lsa\Kerberos\Parameters" -Name "MaxRenewAge" -Value 7 -Type DWord -ErrorAction SilentlyContinue

# WSUS configuration (placeholder - adapt to real WSUS server)
Set-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU" -Name "UseWUServer" -Value 1 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate" -Name "WUServer" -Value "http://wsus.corp.local:8530" -Type String -ErrorAction SilentlyContinue

# RBAC: restrict local admin membership
$allowedAdmins = @("DOMAIN\g-Server-Admins")
$localAdminGroup = Get-LocalGroup -Name "Administrators"
Get-LocalGroupMember -Group $localAdminGroup | Where-Object { $_.Name -notin $allowedAdmins } | ForEach-Object {
    Remove-LocalGroupMember -Group $localAdminGroup -Member $_.Name
}

# Disable SMBv1
Disable-WindowsOptionalFeature -Online -FeatureName SMB1Protocol -NoRestart

# Audit policy
auditpol /set /category:"Logon/Logoff" /success:enable /failure:enable
auditpol /set /category:"Account Management" /success:enable /failure:enable
auditpol /set /category:"Object Access" /success:enable /failure:enable

# Windows Firewall on all profiles
Set-NetFirewallProfile -Profile Domain,Public,Private -Enabled True

Write-Host "Windows hardening applied. Reboot may be required."
