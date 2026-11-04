//! TOOLS-10D: explicit model-facing codec for native browser tools.
//! Pure schema/Protobuf translation; cannot start Chromium or create authority.
//! The host advertises these only for a live authorized task, and native
//! dispatch rechecks the typed payload plus NetworkRead/NetworkWrite effect.
use crate::{
    browser_authorized_execution::{encode_browser_request,decode_browser_request},
    browser_worker_supervisor::BrowserOperation,
    contracts::ProtoPayload,
    model::{ModelToolCodec,ModelToolError},
    wire::proto,
};
use prost::Message;
use serde_json::{json,Map,Value};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum BrowserModelKind {Read,Interact}
pub struct BrowserModelCodec(pub BrowserModelKind);

impl BrowserModelCodec {
    fn select(&self,name:&str)->Result<BrowserOperation,ModelToolError>{
        use BrowserOperation as O;
        match (self.0,name){
            (BrowserModelKind::Read,"navigate")=>Ok(O::Navigate),
            (BrowserModelKind::Read,"read")=>Ok(O::Read),
            (BrowserModelKind::Read,"status")=>Ok(O::Status),
            (BrowserModelKind::Read,"accessibility")=>Ok(O::Accessibility),
            (BrowserModelKind::Read,"screenshot")=>Ok(O::Screenshot),
            (BrowserModelKind::Interact,"click")=>Ok(O::Click),
            (BrowserModelKind::Interact,"click_selector")=>Ok(O::ClickSelector),
            (BrowserModelKind::Interact,"type")=>Ok(O::Type),
            (BrowserModelKind::Interact,"press")=>Ok(O::Press),
            (BrowserModelKind::Interact,"scroll")=>Ok(O::Scroll),
            (BrowserModelKind::Interact,"back")=>Ok(O::Back),
            (BrowserModelKind::Interact,"forward")=>Ok(O::Forward),
            _=>Err(ModelToolError::InvalidArguments("browser operation is not in granted tool class".into()))
        }
    }
    pub fn schema(&self)->Value {
        let operations=match self.0{
            BrowserModelKind::Read=>vec!["navigate","read","status","accessibility","screenshot"],
            BrowserModelKind::Interact=>vec!["click","click_selector","type","press","scroll","back","forward"]
        };
        json!({
          "type":"object",
          "properties":{
            "operation":{"type":"string","enum":operations},
            "url":{"type":"string","maxLength":2048},
            "x":{"type":"integer","minimum":0,"maximum":1280},
            "y":{"type":"integer","minimum":0,"maximum":900},
            "selector":{"type":"string","maxLength":256},
            "text":{"type":"string","maxLength":2048},
            "key":{"type":"string","maxLength":32},
            "delta":{"type":"integer","minimum":-3000,"maximum":3000}
          },
          "required":["operation"],
          "additionalProperties":false
        })
    }
}
impl ModelToolCodec for BrowserModelCodec {
    fn input_type_url(&self)->&'static str {
        "type.googleapis.com/lu.v1.BrowserOperationRequest"
    }
    fn output_type_url(&self)->&'static str {
        "type.googleapis.com/lu.v1.BrowserOperationResult"
    }
    fn encode(&self,args:&Map<String,Value>)->Result<ProtoPayload,ModelToolError>{
        let name=args.get("operation").and_then(Value::as_str)
            .ok_or_else(||ModelToolError::InvalidArguments("missing browser operation".into()))?;
        let op=self.select(name)?;
        let fields: &[&str]=match op {
            BrowserOperation::Navigate=>&["url"],
            BrowserOperation::Click=>&["x","y"],
            BrowserOperation::ClickSelector=>&["selector"],
            BrowserOperation::Type=>&["text"],
            BrowserOperation::Press=>&["key"],
            BrowserOperation::Scroll=>&["delta"],
            _=>&[]
        };
        if args.len()!=fields.len()+1
          ||fields.iter().any(|key|!args.contains_key(*key)){
            return Err(ModelToolError::InvalidArguments("unexpected browser operation fields".into()));
        }
        let bounded=|field:&str,max:usize|{
            args.get(field).and_then(Value::as_str)
                .is_some_and(|x|!x.is_empty()&&x.len()<=max
                    &&!x.chars().any(char::is_control))
        };
        let good=match op{
            BrowserOperation::Navigate=>bounded("url",2048),
            BrowserOperation::Click=>["x","y"].iter().all(|key|
                args.get(*key).and_then(Value::as_u64).is_some_and(|n|
                  n<=if *key=="x"{1280}else{900})),
            BrowserOperation::ClickSelector=>bounded("selector",256),
            BrowserOperation::Type=>bounded("text",2048),
            BrowserOperation::Press=>bounded("key",32)
                &&["Enter","Tab","Escape","Backspace","ArrowUp","ArrowDown",
                    "ArrowLeft","ArrowRight","PageUp","PageDown","Home","End"]
                    .contains(&args.get("key").and_then(Value::as_str).unwrap_or("")),
            BrowserOperation::Scroll=>args.get("delta").and_then(Value::as_i64)
                .is_some_and(|v|(-3000..=3000).contains(&v)),
            _=>true
        };
        if !good{return Err(ModelToolError::InvalidArguments(
            "invalid browser operation argument".into()));}
        let mut payload=args.clone();
        payload.remove("operation");
        encode_browser_request(op,&Value::Object(payload))
            .map_err(|_|ModelToolError::InvalidArguments("browser request failed canonical encoding".into()))
    }
    fn decode(&self,output:&ProtoPayload)->Result<Value,ModelToolError>{
        if output.type_url!=self.output_type_url()||output.bytes.len()>3_000_000 {
            return Err(ModelToolError::InvalidResult("unexpected browser response envelope".into()));
        }
        let result=proto::BrowserOperationResult::decode(output.bytes.as_slice())
            .map_err(|_|ModelToolError::InvalidResult("invalid browser protobuf response".into()))?;
        if result.encode_to_vec()!=output.bytes||result.result_json.len()>2_900_000 {
            return Err(ModelToolError::InvalidResult("noncanonical browser result".into()));
        }
        let value:Value=serde_json::from_slice(&result.result_json)
            .map_err(|_|ModelToolError::InvalidResult("invalid browser result JSON".into()))?;
        if !value.is_object(){return Err(ModelToolError::InvalidResult("browser result must be an object".into()));}
        Ok(json!({"source_is_untrusted":true,"browser_result":value}))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::contracts::ToolInvocation;
    #[test]
    fn read_codec_emits_single_typed_bound_operation(){
        let codec=BrowserModelCodec(BrowserModelKind::Read);
        let map=json!({"operation":"navigate","url":"https://example.com/article"})
            .as_object().unwrap().clone();
        let payload=codec.encode(&map).unwrap();
        let invocation=ToolInvocation{
            call_id:"call".into(),tool_id:"lu.web.browser.read".into(),
            workspace_id:None,input:payload
        };
        let (op,args)=decode_browser_request(&invocation).unwrap();
        assert_eq!(op,BrowserOperation::Navigate);
        assert_eq!(args,json!({"url":"https://example.com/article"}));
        let invalid=json!({"operation":"show_browser"}).as_object().unwrap().clone();
        assert!(codec.encode(&invalid).is_err());
        let invalid=json!({"operation":"navigate","url":"https://example.com","extra":true})
            .as_object().unwrap().clone();
        assert!(codec.encode(&invalid).is_err());
    }
    #[test]
    fn interactive_ops_cannot_be_encoded_as_network_read(){
        let read=BrowserModelCodec(BrowserModelKind::Read);
        let interact=BrowserModelCodec(BrowserModelKind::Interact);
        let click=json!({"operation":"click","x":4,"y":8}).as_object().unwrap().clone();
        assert!(read.encode(&click).is_err());
        assert!(interact.encode(&click).is_ok());
        let bad=json!({"operation":"click","x":-1,"y":4}).as_object().unwrap().clone();
        assert!(interact.encode(&bad).is_err());
        let typing=json!({"operation":"type","text":"hello"}).as_object().unwrap().clone();
        assert!(read.encode(&typing).is_err());
    }
}
