//! Contrôle d'accès par rôles, porté par les équipes et restreint par
//! environnement et par service. Les liaisons sont déclarées dans
//! `socle.yaml` (section `access`), donc versionnées et revues comme du code.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::spec::Spec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Lecture seule.
    Viewer,
    /// Lecture + journal d'audit et rapports de conformité.
    Auditor,
    /// Exploitation courante : retours arrière, sauvegardes, runbooks,
    /// incidents, maintenance, correctifs.
    Operator,
    /// Livraison : déploiements, mise à l'échelle, enregistrement de releases.
    Deployer,
    /// Approbation des changements (principe des quatre yeux).
    Approver,
    /// Administration complète.
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Audit,
    RegisterRelease,
    Deploy,
    Rollback,
    Scale,
    InfraPlan,
    InfraApply,
    Backup,
    Restore,
    Patch,
    Runbook,
    Approve,
    ManageIncidents,
    ManageMaintenance,
    Admin,
}

impl Role {
    pub fn grants(self, p: Permission) -> bool {
        use Permission as P;
        match self {
            Role::Admin => true,
            Role::Viewer => matches!(p, P::Read),
            Role::Auditor => matches!(p, P::Read | P::Audit),
            Role::Operator => matches!(
                p,
                P::Read
                    | P::Rollback
                    | P::Backup
                    | P::Runbook
                    | P::Patch
                    | P::ManageIncidents
                    | P::ManageMaintenance
                    | P::InfraPlan
            ),
            Role::Deployer => matches!(
                p,
                P::Read
                    | P::RegisterRelease
                    | P::Deploy
                    | P::Rollback
                    | P::Scale
                    | P::Backup
                    | P::Runbook
                    | P::ManageIncidents
                    | P::InfraPlan
            ),
            Role::Approver => matches!(p, P::Read | P::Audit | P::Approve | P::ManageIncidents),
        }
    }

    pub const ALL: [Role; 6] =
        [Role::Viewer, Role::Auditor, Role::Operator, Role::Deployer, Role::Approver, Role::Admin];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    /// Utilisateur authentifié par le fournisseur d'identité (OIDC).
    User,
    /// Jeton d'API (CI, automatisations).
    Token,
    /// Processus interne de socle (ordonnanceur).
    System,
    /// Opérateur local (mode `--local` de la CLI).
    Local,
}

/// Identité appelante, après authentification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub display_name: String,
    pub kind: PrincipalKind,
    pub teams: BTreeSet<String>,
}

impl Principal {
    pub fn system(name: &str) -> Self {
        Self {
            id: format!("system:{name}"),
            display_name: format!("socle ({name})"),
            kind: PrincipalKind::System,
            teams: BTreeSet::new(),
        }
    }
}

/// Équipes d'un sujet : appartenance explicite (`members`) ou par groupe
/// du fournisseur d'identité (`idp_groups`).
pub fn resolve_teams(spec: &Spec, subject: &str, idp_groups: &[String]) -> BTreeSet<String> {
    spec.teams
        .iter()
        .filter(|(_, team)| {
            team.members.iter().any(|m| m.eq_ignore_ascii_case(subject))
                || team.idp_groups.iter().any(|g| idp_groups.contains(g))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

fn scope_matches(scope: &[String], value: Option<&str>) -> bool {
    let all = scope.is_empty() || scope.iter().any(|s| s == "*");
    match value {
        None => true,
        Some(v) => all || scope.iter().any(|s| s == v),
    }
}

fn scope_is_global(scope: &[String]) -> bool {
    scope.is_empty() || scope.iter().any(|s| s == "*")
}

/// Vérifie qu'un principal dispose d'une permission. Une dimension absente
/// (`None`) ne restreint pas ; la permission `Admin` exige en revanche une
/// liaison sans restriction d'environnement ni de service.
pub fn is_allowed(
    spec: &Spec,
    principal: &Principal,
    permission: Permission,
    environment: Option<&str>,
    service: Option<&str>,
) -> bool {
    if principal.kind == PrincipalKind::System {
        return true;
    }
    spec.access.iter().any(|b| {
        principal.teams.contains(&b.team)
            && b.role.grants(permission)
            && if permission == Permission::Admin {
                scope_is_global(&b.environments) && scope_is_global(&b.services)
            } else {
                scope_matches(&b.environments, environment) && scope_matches(&b.services, service)
            }
    })
}

/// Rôles effectifs d'un principal (pour l'affichage et la revue d'accès).
pub fn effective_bindings<'a>(spec: &'a Spec, principal: &Principal) -> Vec<&'a crate::spec::AccessBinding> {
    spec.access.iter().filter(|b| principal.teams.contains(&b.team)).collect()
}
