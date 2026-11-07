//! TOOLS-10E: strict streamed llama.cpp /v1/chat/completions tool-call
//! collector. Pure parsing: it cannot execute browser actions or create
//! authority. The caller may route completed, allow-listed function calls to
//! ModelToolCatalog, whose Protobuf and native authorizer remain mandatory.
use std::collections::{BTreeMap,BTreeSet};
use serde_json::Value;

const MAX_SSE_BYTES:usize=64*1024;
const MAX_TOOL_CALLS_PER_COMPLETION:usize=16;
const MAX_TOTAL_TOOL_ARGUMENT_BYTES:usize=512*1024;
const MAX_TOOL_ARGUMENT_BYTES:usize=64*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ToolStreamError{
    InvalidFrame,InvalidSequence,UnadvertisedTool,TooManyCalls,
    OversizedArguments,InvalidArguments,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct CompletedToolCall {
    pub call_id:String,
    pub tool_name:String,
    pub arguments_json:String,
}
#[derive(Debug)]
struct Pending {
    id:Option<String>,
    name:String,
    arguments:String,
}
#[derive(Debug)]
pub enum ToolStreamEvent {
    Fragments(Vec<ToolFragment>),
    EndCalls,
    Stop,
    Done,
    Other,
}
#[derive(Debug)]
pub struct ToolFragment{
    index:usize,
    id:Option<String>,
    name:Option<String>,
    arguments:Option<String>,
}
fn valid_call_id(s:&str)->bool{
    !s.is_empty()&&s.len()<=128
        &&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-')
}
fn tool_name_valid(s:&str)->bool{
    !s.is_empty()&&s.len()<=64&&s.bytes().enumerate().all(|(i,b)|
        b.is_ascii_alphabetic()||i>0&&(b.is_ascii_digit()||b==b'_'))
}
/// Parse only protocol-level tool_calls deltas. Do not scrape prose,
/// markdown or untrusted web text to manufacture function calls.
pub fn parse_tool_sse_frame(bytes:&[u8])->Result<ToolStreamEvent,ToolStreamError>{
    if bytes.len()>MAX_SSE_BYTES{return Err(ToolStreamError::InvalidFrame);}
    let text=std::str::from_utf8(bytes).map_err(|_|ToolStreamError::InvalidFrame)?;
    let mut data=None;
    for line in text.lines().map(str::trim_end){
        if let Some(value)=line.strip_prefix("data: "){
            if data.replace(value).is_some(){return Err(ToolStreamError::InvalidFrame);}
        }else if !line.is_empty()&&!line.starts_with(':')&&!line.starts_with("event:"){
            return Err(ToolStreamError::InvalidFrame);
        }
    }
    let data=data.ok_or(ToolStreamError::InvalidFrame)?;
    if data=="[DONE]"{return Ok(ToolStreamEvent::Done);}
    let object=crate::model_json_boundary::parse_exact_boundary_object(
        data,MAX_SSE_BYTES
    ).map_err(|_|ToolStreamError::InvalidFrame)?;
    if object.contains_key("error"){return Err(ToolStreamError::InvalidFrame);}
    let choice=object.get("choices").and_then(Value::as_array)
        .filter(|a|a.len()==1).and_then(|a|a.first())
        .ok_or(ToolStreamError::InvalidFrame)?;
    let finish=choice.get("finish_reason").and_then(Value::as_str);
    let delta=choice.get("delta");
    if let Some(calls)=delta.and_then(|d|d.get("tool_calls")){
        let array=calls.as_array().filter(|a|!a.is_empty()
            &&a.len()<=MAX_TOOL_CALLS_PER_COMPLETION)
            .ok_or(ToolStreamError::InvalidFrame)?;
        if finish.is_some(){return Err(ToolStreamError::InvalidSequence);}
        let mut parsed=Vec::with_capacity(array.len());
        for call in array{
            let index=call.get("index").and_then(Value::as_u64)
                .and_then(|v|usize::try_from(v).ok())
                .filter(|v|*v<MAX_TOOL_CALLS_PER_COMPLETION)
                .ok_or(ToolStreamError::InvalidFrame)?;
            if call.get("type").and_then(Value::as_str)
                .is_some_and(|v|v!="function"){
                return Err(ToolStreamError::InvalidFrame);
            }
            let id=call.get("id").and_then(Value::as_str).map(str::to_string);
            if id.as_ref().is_some_and(|v|!valid_call_id(v)){
                return Err(ToolStreamError::InvalidFrame);
            }
            let function=call.get("function").and_then(Value::as_object)
                .ok_or(ToolStreamError::InvalidFrame)?;
            let name=function.get("name").and_then(Value::as_str)
                .map(str::to_string);
            let arguments=function.get("arguments").and_then(Value::as_str)
                .map(str::to_string);
            if name.is_none()&&arguments.is_none(){
                return Err(ToolStreamError::InvalidFrame);
            }
            parsed.push(ToolFragment{index,id,name,arguments});
        }
        return Ok(ToolStreamEvent::Fragments(parsed));
    }
    if finish==Some("tool_calls"){return Ok(ToolStreamEvent::EndCalls);}
    if finish==Some("stop"){return Ok(ToolStreamEvent::Stop);}
    if finish.is_some_and(|v|v!="length"){
        return Err(ToolStreamError::InvalidFrame);
    }
    Ok(ToolStreamEvent::Other)
}
pub struct BrowserToolCallCollector {
    allowed:BTreeSet<String>,
    pending:BTreeMap<usize,Pending>,
    done:bool,
    bytes:usize,
}
impl BrowserToolCallCollector{
    pub fn new(allowed:BTreeSet<String>)->Self{
        Self{allowed,pending:BTreeMap::new(),done:false,bytes:0}
    }
    pub fn absorb(&mut self,event:ToolStreamEvent)
        ->Result<Option<Vec<CompletedToolCall>>,ToolStreamError>{
        if self.done {
            return match event{
                ToolStreamEvent::Done=>Ok(None),
                _=>Err(ToolStreamError::InvalidSequence)
            };
        }
        match event {
            ToolStreamEvent::Fragments(parts)=>{
                for part in parts {
                    if !self.pending.contains_key(&part.index)
                        &&self.pending.len()>=MAX_TOOL_CALLS_PER_COMPLETION{
                        return Err(ToolStreamError::TooManyCalls);
                    }
                    let target=self.pending.entry(part.index).or_insert_with(||Pending{
                        id:None,name:String::new(),arguments:String::new()
                    });
                    if let Some(id)=part.id{
                        if target.id.as_deref().is_some_and(|old|old!=id){
                            return Err(ToolStreamError::InvalidSequence);
                        }
                        target.id=Some(id);
                    }
                    if let Some(name)=part.name{
                        target.name.push_str(&name);
                        if target.name.len()>64{return Err(ToolStreamError::InvalidSequence);}
                    }
                    if let Some(args)=part.arguments{
                        self.bytes=self.bytes.saturating_add(args.len());
                        if self.bytes>MAX_TOTAL_TOOL_ARGUMENT_BYTES
                            ||target.arguments.len().saturating_add(args.len())>MAX_TOOL_ARGUMENT_BYTES{
                            return Err(ToolStreamError::OversizedArguments);
                        }
                        target.arguments.push_str(&args);
                    }
                }
                Ok(None)
            }
            ToolStreamEvent::EndCalls=>{
                self.done=true;
                if self.pending.is_empty(){return Err(ToolStreamError::InvalidSequence);}
                let mut result=Vec::new();
                let mut ids=BTreeSet::new();
                for (_,part) in &self.pending{
                    let id=part.id.as_ref().ok_or(ToolStreamError::InvalidSequence)?;
                    if !ids.insert(id.clone())||!tool_name_valid(&part.name){
                        return Err(ToolStreamError::InvalidSequence);
                    }
                    if !self.allowed.contains(&part.name){
                        return Err(ToolStreamError::UnadvertisedTool);
                    }
                    // Do not repair duplicate/invalid fields produced by a
                    // model. The ordinary ModelToolCatalog will revalidate.
                    let json=crate::model_json_boundary::parse_exact_boundary_object(
                        &part.arguments,MAX_TOOL_ARGUMENT_BYTES
                    ).map_err(|_|ToolStreamError::InvalidArguments)?;
                    if json.contains_key("type")||json.contains_key("op"){
                        return Err(ToolStreamError::InvalidArguments);
                    }
                    result.push(CompletedToolCall{
                        call_id:id.clone(),tool_name:part.name.clone(),
                        arguments_json:part.arguments.clone(),
                    });
                }
                Ok(Some(result))
            }
            ToolStreamEvent::Stop=>{
                // Only a completed natural-language stop (not a length
                // cutoff or bare [DONE]) can end the read-only proposal loop.
                if !self.pending.is_empty(){return Err(ToolStreamError::InvalidSequence);}
                self.done=true;Ok(None)
            }
            ToolStreamEvent::Done=>{
                if !self.pending.is_empty(){return Err(ToolStreamError::InvalidSequence);}
                self.done=true;Ok(None)
            }
            ToolStreamEvent::Other=>Ok(None),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(body:&str)->ToolStreamEvent{
        parse_tool_sse_frame(format!("data: {body}\n\n").as_bytes()).unwrap()
    }
    #[test]
    fn fragmented_advertised_function_call_is_assembled_without_executing(){
        let mut allowed=BTreeSet::new();
        allowed.insert("lu_browser_read".into());
        let mut collector=BrowserToolCallCollector::new(allowed);
        let chunk1=r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"lu_browser_read","arguments":"{\"operation\":\"navigate\","}}]}}]}"#;
        let chunk2=r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"url\":\"https://example.com\"}"}}]}}]}"#;
        assert!(collector.absorb(frame(chunk1)).unwrap().is_none());
        assert!(collector.absorb(frame(chunk2)).unwrap().is_none());
        let finished=collector.absorb(frame(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#))
            .unwrap().unwrap();
        assert_eq!(finished.len(),1);
        assert_eq!(finished[0].tool_name,"lu_browser_read");
        assert_eq!(finished[0].arguments_json,
            r#"{"operation":"navigate","url":"https://example.com"}"#);
    }
    #[test]
    fn clean_stop_is_distinct_from_truncated_or_bare_done(){
        let allowed=["lu_browser_read".into()].into();
        let mut collector=BrowserToolCallCollector::new(allowed);
        assert!(matches!(frame(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#),
            ToolStreamEvent::Stop));
        collector.absorb(frame(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#)).unwrap();
        assert!(collector.absorb(ToolStreamEvent::Done).unwrap().is_none());
        let mut half=BrowserToolCallCollector::new(["lu_browser_read".into()].into());
        half.absorb(frame(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"lu_browser_read","arguments":"{"}}]}}]}"#)).unwrap();
        assert!(matches!(half.absorb(frame(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#)),
            Err(ToolStreamError::InvalidSequence)));
        assert!(matches!(frame(r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#),
            ToolStreamEvent::Other));
    }
    #[test]
    fn no_unadvertised_tool_or_duplicate_json_survives_finish(){
        let mut allowed=BTreeSet::new();
        allowed.insert("lu_browser_read".into());
        let mut collector=BrowserToolCallCollector::new(allowed);
        let unwanted=r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"lu_browser_interact","arguments":"{}"}}]}}]}"#;
        collector.absorb(frame(unwanted)).unwrap();
        assert!(matches!(collector.absorb(frame(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#)),
            Err(ToolStreamError::UnadvertisedTool)));
        let mut collector=BrowserToolCallCollector::new(["lu_browser_read".into()].into());
        let bad=r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"lu_browser_read","arguments":"{\"operation\":\"read\",\"operation\":\"click\"}"}}]}}]}"#;
        collector.absorb(frame(bad)).unwrap();
        assert!(matches!(collector.absorb(frame(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#)),
            Err(ToolStreamError::InvalidArguments)));
    }
}
