//! TOOLS-10E: host-validated llama.cpp function-call request construction.
//! This builds a model request; it does NOT create a browser grant or run a
//! tool. The caller supplies schemas from the already authorized task catalog.
use crate::{
    inference_host::ChatTurn,
    multimodal_saved_turn::{build_structured_chat_request,PinnedImage},
};
use serde_json::{Value,json};
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum BrowserToolRequestError{
    InvalidRequest,UnadvertisedTool,InvalidSchema,
}
/// The first browser integration is strictly read-only. Interactions require
/// a separately approved NetworkWrite task before any future expansion.
pub fn build_read_only_browser_tool_request(
    prompt:&str,
    history:&[ChatTurn],
    images:&[PinnedImage],
    max_new_tokens:u32,
    advertised:&[Value],
)->Result<String,BrowserToolRequestError>{
    let mut body:Value=serde_json::from_str(
        &build_structured_chat_request(prompt,history,images,max_new_tokens)
            .map_err(|_|BrowserToolRequestError::InvalidRequest)?
    ).map_err(|_|BrowserToolRequestError::InvalidRequest)?;
    if advertised.len()!=1{return Err(BrowserToolRequestError::UnadvertisedTool);}
    let schema=&advertised[0];
    let name=schema.pointer("/function/name").and_then(Value::as_str);
    if name!=Some("lu_browser_read")
      ||schema.get("type").and_then(Value::as_str)!=Some("function")
      ||schema.pointer("/function/parameters/type").and_then(Value::as_str)!=Some("object")
      ||schema.pointer("/function/parameters/additionalProperties")!=Some(&json!(false)){
        return Err(BrowserToolRequestError::InvalidSchema);
    }
    let enum_values=schema.pointer("/function/parameters/properties/operation/enum")
        .and_then(Value::as_array).ok_or(BrowserToolRequestError::InvalidSchema)?;
    let expected=["navigate","read","status","accessibility"];
    if enum_values.len()!=expected.len()||expected.iter().any(|name|
        !enum_values.iter().any(|v|v.as_str()==Some(*name))){
        return Err(BrowserToolRequestError::InvalidSchema);
    }
    let body=body.as_object_mut().ok_or(BrowserToolRequestError::InvalidRequest)?;
    body.insert("tools".into(),Value::Array(advertised.to_vec()));
    body.insert("tool_choice".into(),json!("auto"));
    body.insert("parallel_tool_calls".into(),json!(false));
    let encoded=serde_json::to_string(body)
        .map_err(|_|BrowserToolRequestError::InvalidRequest)?;
    if encoded.len()>48*1024*1024{
        return Err(BrowserToolRequestError::InvalidRequest);
    }
    Ok(encoded)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::browser_model_codec::{BrowserModelCodec,BrowserModelKind};
    #[test]
    fn only_read_tool_is_in_advertised_llama_completion(){
        let schema=BrowserModelCodec(BrowserModelKind::Read).schema();
        let tools=vec![json!({"type":"function","function":{
            "name":"lu_browser_read","description":"Read approved sources",
            "parameters":schema
        }})];
        let body=build_read_only_browser_tool_request(
            "Find verified sources",&[],&[],256,&tools).unwrap();
        let data:Value=serde_json::from_str(&body).unwrap();
        assert_eq!(data["tools"][0]["function"]["name"],"lu_browser_read");
        assert_eq!(data["tool_choice"],"auto");
        assert_eq!(data["parallel_tool_calls"],false);
        assert_eq!(data["messages"][0]["role"],"user");
        assert!(body.len()<20000);
        assert_eq!(data["tools"][0]["function"]["parameters"]["properties"]["operation"]["enum"]
            .as_array().unwrap().len(),4);
        assert!(!body.contains("\\"screenshot\\""));

        let mut bad=tools.clone();
        bad.push(json!({"type":"function","function":{
            "name":"lu_browser_interact","parameters":{}
        }}));
        assert_eq!(build_read_only_browser_tool_request(
            "Find verified sources",&[],&[],256,&bad),
            Err(BrowserToolRequestError::UnadvertisedTool));
    }
}
