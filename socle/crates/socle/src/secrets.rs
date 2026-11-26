//! Résolution des références de secrets au moment de l'exécution. socle ne
//! stocke aucun secret : il interroge le coffre du fournisseur concerné
//! (multi-cloud) puis masque la valeur dans tous les journaux.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

#[derive(Debug, Clone, Default)]
pub struct SecretResolver;

impl SecretResolver {
    pub async fn resolve(&self, reference: &str) -> Result<String, String> {
        if let Some(name) = reference.strip_prefix("env:") {
            return std::env::var(name).map_err(|_| format!("variable d'environnement {name} absente"));
        }
        if let Some(path) = reference.strip_prefix("file:") {
            return tokio::fs::read_to_string(path)
                .await
                .map(|s| s.trim_end_matches(['\n', '\r']).to_string())
                .map_err(|e| format!("lecture du secret {path} impossible : {e}"));
        }
        if let Some(rest) = reference.strip_prefix("vault:") {
            let (path, field) = rest.split_once('#').ok_or("format attendu : vault:<chemin>#<champ>")?;
            return run_cli("vault", &["kv".into(), "get".into(), format!("-field={field}"), path.into()]).await;
        }
        if let Some(rest) = reference.strip_prefix("aws-sm:") {
            let (id, key) = match rest.split_once('#') {
                Some((id, key)) => (id, Some(key)),
                None => (rest, None),
            };
            let raw = run_cli(
                "aws",
                &[
                    "secretsmanager".into(),
                    "get-secret-value".into(),
                    format!("--secret-id={id}"),
                    "--query=SecretString".into(),
                    "--output=text".into(),
                ],
            )
            .await?;
            return match key {
                None => Ok(raw),
                Some(k) => serde_json::from_str::<serde_json::Value>(&raw)
                    .ok()
                    .and_then(|v| v.get(k).and_then(|x| x.as_str()).map(str::to_string))
                    .ok_or_else(|| format!("clé {k} absente du secret {id}")),
            };
        }
        if let Some(rest) = reference.strip_prefix("azure-kv:") {
            let (vault, name) = rest.split_once('/').ok_or("format attendu : azure-kv:<coffre>/<nom>")?;
            return run_cli(
                "az",
                &[
                    "keyvault".into(),
                    "secret".into(),
                    "show".into(),
                    format!("--vault-name={vault}"),
                    format!("--name={name}"),
                    "--query=value".into(),
                    "--output=tsv".into(),
                ],
            )
            .await;
        }
        if let Some(rest) = reference.strip_prefix("gcp-sm:") {
            let mut parts = rest.splitn(3, '/');
            let (Some(project), Some(name)) = (parts.next(), parts.next()) else {
                return Err("format attendu : gcp-sm:<projet>/<secret>[/<version>]".into());
            };
            let version = parts.next().unwrap_or("latest");
            return run_cli(
                "gcloud",
                &[
                    "secrets".into(),
                    "versions".into(),
                    "access".into(),
                    version.into(),
                    format!("--secret={name}"),
                    format!("--project={project}"),
                ],
            )
            .await;
        }
        Err("type de référence de secret inconnu".into())
    }
}

async fn run_cli(program: &str, args: &[String]) -> Result<String, String> {
    let fut = Command::new(program).args(args).stdin(Stdio::null()).kill_on_drop(true).output();
    let out = tokio::time::timeout(Duration::from_secs(30), fut)
        .await
        .map_err(|_| format!("{program} : délai dépassé"))?
        .map_err(|e| format!("{program} : lancement impossible ({e})"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{program} : {}", err.lines().next().unwrap_or("échec")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn env_and_file_references() {
        let r = SecretResolver;
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("token");
        std::fs::write(&path, "s3cr3t\n").expect("écriture");
        assert_eq!(r.resolve(&format!("file:{}", path.display())).await.expect("file"), "s3cr3t");
        assert!(r.resolve("env:SOCLE_TEST_SECRET_THAT_DOES_NOT_EXIST").await.is_err());
        assert!(r.resolve("plain-value").await.is_err());
        assert!(r.resolve("vault:secret/app").await.is_err(), "champ manquant");
    }
}
