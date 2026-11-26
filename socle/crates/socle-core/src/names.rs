//! Règles de nommage et de format. Toute valeur qui finit dans un chemin, un
//! argument de commande ou une URL passe par ces contrôles : c'est la première
//! barrière contre l'injection (les commandes ne passent jamais par un shell).

use std::sync::LazyLock;

use regex::Regex;

static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]([a-z0-9-]{0,61}[a-z0-9])?$").expect("regex"));
static DIGEST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^sha256:[a-f0-9]{64}$").expect("regex"));
static IMAGE_REPO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]([a-z0-9._-]*[a-z0-9])?(:[0-9]{1,5})?(/[a-z0-9]([a-z0-9._-]*[a-z0-9])?)+$").expect("regex")
});
static ANSIBLE_GROUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_-]{0,127}$").expect("regex"));
static KUBE_CONTEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._:@/-]{0,252}$").expect("regex"));
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$").expect("regex"));
static GIT_SHA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-f0-9]{7,64}$").expect("regex"));
static BATCH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([1-9][0-9]{0,3}|([1-9][0-9]?|100)%)$").expect("regex"));

/// Nom d'objet socle (environnement, service, cible…) : étiquette DNS.
pub fn is_valid_name(s: &str) -> bool {
    NAME.is_match(s)
}

pub fn is_valid_digest(s: &str) -> bool {
    DIGEST.is_match(s)
}

/// Dépôt d'image sans tag ni digest (ex. `ghcr.io/org/app`).
pub fn is_valid_image_repo(s: &str) -> bool {
    IMAGE_REPO.is_match(s)
}

pub fn is_valid_ansible_group(s: &str) -> bool {
    ANSIBLE_GROUP.is_match(s)
}

/// Contexte kubeconfig (les ARN EKS contiennent `:` et `/`). Le premier
/// caractère ne peut pas être `-`, ce qui interdit toute confusion avec une
/// option de ligne de commande.
pub fn is_valid_kube_context(s: &str) -> bool {
    KUBE_CONTEXT.is_match(s)
}

pub fn is_valid_version(s: &str) -> bool {
    VERSION.is_match(s)
}

pub fn is_valid_git_sha(s: &str) -> bool {
    GIT_SHA.is_match(s)
}

/// Taille de lot Ansible : nombre d'hôtes (`2`) ou pourcentage (`25%`).
pub fn is_valid_batch(s: &str) -> bool {
    BATCH.is_match(s)
}

/// Chemin relatif sûr : non vide, pas absolu, sans `..`, sans antislash ni
/// lettre de lecteur. Les gabarits `{env}`/`{service}` sont autorisés.
pub fn is_safe_relative_path(p: &str) -> bool {
    if p.is_empty() || p.starts_with('/') || p.starts_with('-') || p.contains('\\') || p.contains('\0') {
        return false;
    }
    if p.len() >= 2 && p.as_bytes()[1] == b':' {
        return false;
    }
    !p.split('/').any(|part| part == "..")
}

/// Chemin absolu POSIX sur un hôte géré (sauvegarde de fichiers).
pub fn is_safe_absolute_path(p: &str) -> bool {
    p.starts_with('/') && !p.contains('\0') && !p.split('/').any(|part| part == "..")
}

pub fn is_http_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://")) && !s.contains(char::is_whitespace)
}

/// Préfixes de références de secrets acceptés. socle ne stocke jamais de
/// secret : il les résout au moment de l'exécution.
pub const SECRET_PREFIXES: [&str; 6] = ["env:", "file:", "vault:", "aws-sm:", "azure-kv:", "gcp-sm:"];

pub fn is_secret_ref(s: &str) -> bool {
    SECRET_PREFIXES.iter().any(|p| s.starts_with(p) && s.len() > p.len())
}

/// Remplace les gabarits `{env}` et `{service}`.
pub fn expand(template: &str, env: &str, service: &str) -> String {
    template.replace("{env}", env).replace("{service}", service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_valid_name("erp-app"));
        assert!(is_valid_name("prod"));
        assert!(!is_valid_name("Prod"));
        assert!(!is_valid_name("-x"));
        assert!(!is_valid_name("x-"));
        assert!(!is_valid_name("a_b"));
        assert!(!is_valid_name(""));
    }

    #[test]
    fn images_and_digests() {
        assert!(is_valid_image_repo("ghcr.io/reyptz/lab-sysadmin-sec/erp-app"));
        assert!(is_valid_image_repo("registry.local:5000/team/app"));
        assert!(!is_valid_image_repo("ghcr.io/org/app:1.0"));
        assert!(!is_valid_image_repo("app"));
        assert!(is_valid_digest(&format!("sha256:{}", "a".repeat(64))));
        assert!(!is_valid_digest("sha256:abc"));
    }

    #[test]
    fn paths() {
        assert!(is_safe_relative_path("gitops/{env}"));
        assert!(is_safe_relative_path("."));
        assert!(!is_safe_relative_path("../etc"));
        assert!(!is_safe_relative_path("a/../../b"));
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("C:/Windows"));
        assert!(!is_safe_relative_path("a\\b"));
        assert!(!is_safe_relative_path("--help"));
        assert!(is_safe_absolute_path("/var/www"));
        assert!(!is_safe_absolute_path("/var/../etc"));
    }

    #[test]
    fn misc() {
        assert!(is_valid_batch("1"));
        assert!(is_valid_batch("25%"));
        assert!(is_valid_batch("100%"));
        assert!(!is_valid_batch("0"));
        assert!(!is_valid_batch("150%"));
        assert!(is_secret_ref("env:SLACK_WEBHOOK"));
        assert!(!is_secret_ref("https://hooks.slack.com/x"));
        assert!(!is_secret_ref("env:"));
        assert!(is_valid_kube_context("arn:aws:eks:eu-west-3:123:cluster/prod"));
        assert!(!is_valid_kube_context("--kubeconfig=/tmp/x"));
        assert_eq!(expand("{service}-{env}", "prod", "erp"), "erp-prod");
    }
}
