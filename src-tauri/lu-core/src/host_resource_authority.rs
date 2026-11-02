//! Deterministic resource authorization. Similarity/HDC retrieval can propose
//! candidate resources, but MUST NOT construct or approve an access grant.
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sensitivity {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAction {
    Read,
    Export,
    Train,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRecord {
    pub conversation_id: String,
    pub owner_profile_id: String,
    pub project_id: String,
    pub sensitivity: Sensitivity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPrincipal {
    pub profile_id: String,
    pub active_project_id: String,
    /// Granted separately by host workflow (never by model output).
    pub export_enabled: bool,
    pub training_enabled: bool,
    pub maximum_sensitivity: Sensitivity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceDenial {
    MissingRecord,
    InvalidIdentity,
    WrongOwner,
    WrongProject,
    SensitivityExceeded,
    MissingCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedResource {
    conversation_id: String,
    project_id: String,
    action: ResourceAction,
}

impl AuthorizedResource {
    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    pub fn action(&self) -> ResourceAction {
        self.action
    }
}

#[derive(Default)]
pub struct HostResourceAuthority {
    resources: BTreeMap<String, ResourceRecord>,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

impl HostResourceAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    /// Host-enrolled, immutable within this instance. Do not accept records
    /// straight from imported JSON, webview, model text, or HDC candidates.
    pub fn enroll(&mut self, record: ResourceRecord) -> Result<(), ResourceDenial> {
        if !valid_id(&record.conversation_id)
            || !valid_id(&record.owner_profile_id)
            || !valid_id(&record.project_id)
        {
            return Err(ResourceDenial::InvalidIdentity);
        }
        match self.resources.get(&record.conversation_id) {
            Some(existing) if existing != &record => Err(ResourceDenial::WrongOwner),
            Some(_) => Ok(()),
            None => {
                self.resources
                    .insert(record.conversation_id.clone(), record);
                Ok(())
            }
        }
    }

    /// Exact, deterministic policy evaluation; fail closed on unknown records.
    pub fn require(
        &self,
        principal: &HostPrincipal,
        conversation_id: &str,
        action: ResourceAction,
    ) -> Result<AuthorizedResource, ResourceDenial> {
        if !valid_id(conversation_id)
            || !valid_id(&principal.profile_id)
            || !valid_id(&principal.active_project_id)
        {
            return Err(ResourceDenial::InvalidIdentity);
        }
        let record = self
            .resources
            .get(conversation_id)
            .ok_or(ResourceDenial::MissingRecord)?;
        if record.owner_profile_id != principal.profile_id {
            return Err(ResourceDenial::WrongOwner);
        }
        if record.project_id != principal.active_project_id {
            return Err(ResourceDenial::WrongProject);
        }
        if record.sensitivity > principal.maximum_sensitivity {
            return Err(ResourceDenial::SensitivityExceeded);
        }
        match action {
            ResourceAction::Read => {}
            ResourceAction::Export if principal.export_enabled => {}
            ResourceAction::Train if principal.training_enabled => {}
            _ => return Err(ResourceDenial::MissingCapability),
        }
        Ok(AuthorizedResource {
            conversation_id: record.conversation_id.clone(),
            project_id: record.project_id.clone(),
            action,
        })
    }
}
