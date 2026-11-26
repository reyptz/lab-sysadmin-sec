//! **socle** — plan de contrôle du cycle de vie des déploiements applicatifs
//! et des infrastructures : déploiement, CI/CD, supervision, scalabilité,
//! environnements, sauvegardes et restaurations, sécurité, incidents et
//! maintenance, en multi-cloud (AWS, Azure, GCP, OVHcloud, Scaleway) et
//! on-premise (RHEL), sur Kubernetes comme sur VMs.
//!
//! Architecture hexagonale : le noyau métier (`socle-core`) est pur ; ce
//! crate fournit les adaptateurs (persistance, exécution, notifications,
//! authentification) et les trois interfaces (API REST, console web, CLI),
//! qui partagent strictement le même moteur.

pub mod api;
pub mod auth;
pub mod cli;
pub mod console;
pub mod engine;
pub mod error;
pub mod executor;
pub mod model;
pub mod notify;
pub mod secrets;
pub mod store;

pub use error::{Error, Result};
