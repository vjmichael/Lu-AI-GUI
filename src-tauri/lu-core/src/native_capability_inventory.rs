//! Wave 3: typed, authority-aware discovery of *already registered* native
//! model tools. Discovery never opens a grant, executes a tool, or turns a
//! public schema/extension manifest into runtime authority.
use std::collections::BTreeSet;
use crate::{
    contracts::ToolEffect,
    conversation_authority::{BoundCallOrigin,ConversationAuthority},
    model::ModelToolCatalog,
    registry::NativeToolRegistry,
};
#[derive(Clone,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct NativeCapabilityView{
    pub native_tool_id:String,
    pub public_name:String,
    pub display_name:String,
    pub effect:String,
    pub input_type_url:String,
    pub output_type_url:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum CapabilityInventoryError{
    AuthorityUnavailable,RegistryUnavailable,ModelCatalogUnavailable,
    MissingNativeContract,PrivilegeEscalation,
}
fn effect_label(effect:ToolEffect)->&'static str{
    match effect{
        ToolEffect::ReadWorkspace=>"read_workspace",
        ToolEffect::WriteWorkspace=>"write_workspace",
        ToolEffect::ExecuteCode=>"execute_code",
        ToolEffect::NetworkRead=>"network_read",
        ToolEffect::NetworkWrite=>"network_write",
        ToolEffect::DeviceControl=>"device_control",
        ToolEffect::Elevated=>"elevated",
    }
}
/// Only exact, *currently permitted* native IDs with a matching installed
/// codec are projected. This is not a global ability catalog. A revoked
/// or unregistered tool is absent rather than advertised with fake access.
pub fn discover_turn_capabilities(
    registry:&NativeToolRegistry,catalog:&ModelToolCatalog,
    authority:&ConversationAuthority,origin:&BoundCallOrigin,
    only_read:bool,
)->Result<Vec<NativeCapabilityView>,CapabilityInventoryError>{
    let allowed=authority.permitted_tool_ids(origin)
        .map_err(|_|CapabilityInventoryError::AuthorityUnavailable)?;
    let descriptors=registry.tools()
        .map_err(|_|CapabilityInventoryError::RegistryUnavailable)?;
    let names=catalog.names_for_native_ids(&allowed);
    let schemas=catalog.schemas(&names)
        .map_err(|_|CapabilityInventoryError::ModelCatalogUnavailable)?;
    let schema_names=schemas.iter().filter_map(|s|
        s.pointer("/function/name").and_then(serde_json::Value::as_str)
    ).collect::<BTreeSet<_>>();
    let mut result=Vec::new();
    for desc in descriptors{
        if !allowed.contains(&desc.id){continue}
        if only_read && desc.effect!=ToolEffect::ReadWorkspace{
            return Err(CapabilityInventoryError::PrivilegeEscalation)
        }
        let registered=catalog.names_for_native_ids(&BTreeSet::from([desc.id.clone()]));
        if registered.is_empty(){
            return Err(CapabilityInventoryError::MissingNativeContract)
        }
        for public_name in registered{
            if !schema_names.contains(public_name.as_str()){
                return Err(CapabilityInventoryError::MissingNativeContract)
            }
            result.push(NativeCapabilityView{
                native_tool_id:desc.id.clone(),
                public_name,
                display_name:desc.display_name.clone(),
                effect:effect_label(desc.effect).into(),
                input_type_url:desc.input_type_url.clone(),
                output_type_url:desc.output_type_url.clone(),
            });
        }
    }
    // An allowed registered model tool cannot disappear silently from the
    // published inventory while leaving an exposed inference schema behind.
    if result.len()!=schema_names.len(){
        return Err(CapabilityInventoryError::MissingNativeContract)
    }
    result.sort_by(|a,b|a.public_name.cmp(&b.public_name));
    Ok(result)
}
