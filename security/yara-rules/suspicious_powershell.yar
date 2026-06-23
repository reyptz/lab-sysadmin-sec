rule Suspicious_PowerShell_Download
{
    meta:
        description = "Detects PowerShell download cradle patterns"
        author = "lab-sysadmin-sec"
        date = "2026-06-23"
    strings:
        $a = "Invoke-Expression" ascii wide nocase
        $b = "IEX" ascii wide nocase
        $c = "Net.WebClient" ascii wide nocase
        $d = "DownloadString" ascii wide nocase
        $e = "DownloadFile" ascii wide nocase
        $f = "bitsadmin" ascii wide nocase
    condition:
        any of ($a, $b) and any of ($c, $d, $e, $f)
}

rule PowerShell_Base64_Encoded
{
    meta:
        description = "Detects base64 encoded PowerShell commands"
        author = "lab-sysadmin-sec"
    strings:
        $ps = "powershell" ascii wide nocase
        $enc = "-encodedcommand" ascii wide nocase
        $enc2 = "-enc" ascii wide nocase
        $b64 = /[A-Za-z0-9+\/]{100,}={0,2}/
    condition:
        ($ps or $enc or $enc2) and $b64
}
