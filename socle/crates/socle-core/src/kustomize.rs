//! Édition ciblée d'un `kustomization.yaml` : seule l'entrée `images:` du
//! service est réécrite, le reste du fichier (commentaires, ordre, patches)
//! est conservé à l'identique. Le résultat est relu pour contrôle.

/// Fixe l'image `name` sur `new_name@digest` (supprime `newTag`).
pub fn set_image(content: &str, name: &str, new_name: &str, digest: &str) -> Result<String, String> {
    let trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

    let entry = |indent: &str| {
        vec![
            format!("{indent}- name: {name}"),
            format!("{indent}  newName: {new_name}"),
            format!("{indent}  digest: {digest}"),
        ]
    };

    let images_idx = lines.iter().position(|l| {
        let t = l.trim_end();
        t == "images:" || t.starts_with("images: ") || t.starts_with("images:#")
    });

    match images_idx {
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("images:".into());
            lines.extend(entry("  "));
        }
        Some(start) => {
            // Forme en ligne (`images: []`) : remplacée par un bloc.
            if lines[start].trim_end() != "images:" {
                let rest = lines[start].trim_end()["images:".len()..].trim().to_string();
                if !(rest.is_empty() || rest == "[]" || rest.starts_with('#')) {
                    return Err("images: en ligne non vide non pris en charge".into());
                }
                lines[start] = "images:".into();
            }
            // Fin du bloc : prochaine clé de premier niveau.
            let end = lines[start + 1..]
                .iter()
                .position(|l| {
                    let first = l.chars().next();
                    first.is_some_and(|c| c != ' ' && c != '-' && c != '#' && c != '\t')
                })
                .map_or(lines.len(), |p| start + 1 + p);

            // Repère les entrées de liste (`- ...`) à l'indentation minimale.
            let items: Vec<(usize, usize)> = lines[start + 1..end]
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let indent = l.len() - l.trim_start().len();
                    l.trim_start().starts_with('-').then_some((start + 1 + i, indent))
                })
                .collect();
            let dash_indent = items.iter().map(|(_, i)| *i).min();
            let items: Vec<usize> =
                items.into_iter().filter(|(_, i)| Some(*i) == dash_indent).map(|(idx, _)| idx).collect();
            let dash = dash_indent.unwrap_or(2);
            let indent = " ".repeat(dash);

            // Fin d'une entrée : dernière ligne non vide plus indentée que le
            // tiret. Un commentaire ou une clé au niveau du tiret (ou moins)
            // appartient à la suite du fichier et doit être conservé.
            let item_end = |item_start: usize| {
                let mut last = item_start + 1;
                for (j, l) in lines.iter().enumerate().take(end).skip(item_start + 1) {
                    if l.trim().is_empty() {
                        continue;
                    }
                    if l.len() - l.trim_start().len() > dash {
                        last = j + 1;
                    } else {
                        break;
                    }
                }
                last
            };

            let mut found = None;
            for &item_start in &items {
                let item_stop = item_end(item_start);
                let has_name = lines[item_start..item_stop].iter().any(|l| {
                    let t = l.trim_start().trim_start_matches('-').trim();
                    t.strip_prefix("name:").map(|v| v.trim().trim_matches(['"', '\'']) == name).unwrap_or(false)
                });
                if has_name {
                    found = Some((item_start, item_stop));
                    break;
                }
            }
            match found {
                Some((a, b)) => {
                    lines.splice(a..b, entry(&indent));
                }
                None => {
                    let insert_at = items.last().map_or(start + 1, |&last| item_end(last));
                    lines.splice(insert_at..insert_at, entry(&indent));
                }
            }
        }
    }

    let mut out = lines.join("\n");
    if trailing_newline || images_idx.is_none() {
        out.push('\n');
    }
    // Contrôle : le YAML produit est valide et porte bien le digest attendu.
    match read_image(&out, name) {
        Some(img) if img.digest.as_deref() == Some(digest) && img.new_name.as_deref() == Some(new_name) => Ok(out),
        _ => Err("la modification du kustomization.yaml n'a pas pu être vérifiée".into()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KustomizeImage {
    pub new_name: Option<String>,
    pub new_tag: Option<String>,
    pub digest: Option<String>,
}

/// Lit l'entrée `images:` d'un service (détection de dérive).
pub fn read_image(content: &str, name: &str) -> Option<KustomizeImage> {
    let doc: serde_norway::Value = serde_norway::from_str(content).ok()?;
    let images = doc.get("images")?.as_sequence()?;
    let img = images.iter().find(|i| i.get("name").and_then(|n| n.as_str()) == Some(name))?;
    let s = |k: &str| img.get(k).and_then(|v| v.as_str()).map(str::to_string);
    Some(KustomizeImage {
        new_name: s("newName"),
        new_tag: s("newTag").or_else(|| img.get("newTag").and_then(|v| v.as_i64()).map(|n| n.to_string())),
        digest: s("digest"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    #[test]
    fn updates_existing_entry_and_keeps_comments() {
        let input = "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\n\nnamespace: erp-dev\n\nresources:\n  - ../../base\n\n# Le tag est mis à jour par socle.\nimages:\n  - name: erp-app\n    newTag: dev-1.0.0\n\npatches:\n  - path: socle-scaling.yaml\n";
        let out = set_image(input, "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("édition");
        assert!(out.contains("# Le tag est mis à jour par socle."));
        assert!(!out.contains("newTag"));
        assert!(out.contains(&format!("    digest: {DIGEST}")));
        assert!(out.contains("patches:\n  - path: socle-scaling.yaml"));
        // Idempotence.
        assert_eq!(set_image(&out, "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("idem"), out);
    }

    /// Régression : un commentaire en colonne 0 et la ligne vide qui suivent
    /// le bloc `images:` appartiennent à la section suivante (cas réel de
    /// l'overlay dev, révélé par le test de bout en bout).
    #[test]
    fn keeps_following_section_comment_and_blank_line() {
        let input = "images:\n  - name: erp-app\n    newTag: dev-1.0.0\n\n# Bornes d'autoscaling gérées par socle.\npatches:\n  - path: socle-scaling.yaml\n";
        let out = set_image(input, "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("édition");
        assert!(
            out.ends_with(&format!(
                "    digest: {DIGEST}\n\n# Bornes d'autoscaling gérées par socle.\npatches:\n  - path: socle-scaling.yaml\n"
            )),
            "{out}"
        );
        // Ajout d'une nouvelle image : insérée après la dernière entrée, avant le commentaire.
        let out = set_image(&out, "sidecar", "ghcr.io/acme/sidecar", DIGEST).expect("ajout");
        assert!(out.contains("  - name: sidecar\n    newName: ghcr.io/acme/sidecar\n"), "{out}");
        assert!(out.contains("\n\n# Bornes d'autoscaling"), "{out}");
        assert!(read_image(&out, "erp-app").is_some());
    }

    #[test]
    fn handles_unindented_lists_and_other_images() {
        let input = "images:\n- name: sidecar\n  newTag: \"2\"\n- name: erp-app\n  newName: old\n  newTag: x\nresources:\n- a.yaml\n";
        let out = set_image(input, "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("édition");
        let side = read_image(&out, "sidecar").expect("sidecar");
        assert_eq!(side.new_tag.as_deref(), Some("2"));
        let app = read_image(&out, "erp-app").expect("app");
        assert_eq!(app.digest.as_deref(), Some(DIGEST));
        assert!(out.contains("resources:\n- a.yaml"));
    }

    #[test]
    fn appends_when_missing() {
        let out = set_image("resources:\n  - ../../base\n", "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("ajout");
        assert!(out.ends_with(&format!(
            "images:\n  - name: erp-app\n    newName: ghcr.io/acme/erp-app\n    digest: {DIGEST}\n"
        )));
        let out =
            set_image("images: []\nresources: []\n", "erp-app", "ghcr.io/acme/erp-app", DIGEST).expect("liste vide");
        assert!(read_image(&out, "erp-app").is_some());
    }
}
