# OSINT — Google Dorks

Collection de dorks utiles pour l'audit externe du lab.

## Recherche de documents sensibles

```
site:corp.local ext:pdf | ext:doc | ext:docx | ext:xls | ext:xlsx
site:corp.local intitle:"index of" "config"
site:corp.local filetype:env
site:corp.local filetype:sql
```

## Exposition de services

```
site:corp.local inurl:admin
site:corp.local inurl:login
site:corp.local inurl:phpmyadmin
site:corp.local intitle:"Apache2 Ubuntu Default Page"
```

## GitHub/GitLab leaks

```
"corp.local" "api_key" "api_secret" github.com
"corp.local" "password" "BEGIN RSA PRIVATE KEY"
```

## Maltego basics

- Transformations standard: DNS, Whois, Netblock.
- Person: email → social profiles.
- Domain → subdomains via transforms publics.

## Sources

- https://www.exploit-db.com/google-hacking-database
- https://github.com/TurakhiaLab/pans
