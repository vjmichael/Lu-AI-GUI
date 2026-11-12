//! TEST-ONLY child emulating a subset of KoboldCpp's authenticated local
//! HTTP API. It is NOT a model engine and emits no GGUF-derived content.
//! Compiled only with p7l-native-fixture; never installed with normal Lu.
use std::{
    env,fs::File,io::{BufRead,BufReader,Read,Write},
    net::{Ipv4Addr,TcpListener,TcpStream},
    path::PathBuf,thread,time::Duration,
};
use serde_json::json;

fn parse()->Result<(PathBuf,u16,String),String>{
    let args=env::args().skip(1).collect::<Vec<_>>();
    let mut model=None;
    let mut port=None;
    let mut host=None;
    let mut use_cpu=false;
    let mut layers=None;
    let mut context=None;
    let mut threads=None;
    let mut i=0;
    while i<args.len(){
        let key=args[i].as_str();
        match key {
            "--quiet"=>{i+=1},
            "--usecpu"=>{use_cpu=true;i+=1},
            "--model"|"--contextsize"|"--threads"|
            "--gpulayers"|"--host"|"--port"|"--mmproj"=>{
                let value=args.get(i+1).ok_or("missing flag value")?;
                match key {
                    "--model"=>model=Some(PathBuf::from(value)),
                    "--contextsize"=>context=value.parse::<u32>().ok(),
                    "--threads"=>threads=value.parse::<u32>().ok(),
                    "--gpulayers"=>layers=value.parse::<u32>().ok(),
                    "--host"=>host=Some(value.clone()),
                    "--port"=>port=value.parse::<u16>().ok(),
                    "--mmproj"=>{
                        if !PathBuf::from(value).is_file(){
                            return Err("missing original test projector".into())
                        }
                    }
                    _=>{}
                }
                i+=2
            }
            _=>return Err("unsupported test fixture command flag".into()),
        }
    }
    let model=model.ok_or("missing original test model")?;
    let mut gguf=File::open(&model).map_err(|_|"model file unavailable")?;
    let mut magic=[0u8;4];
    gguf.read_exact(&mut magic).map_err(|_|"short model")?;
    if magic!=*b"GGUF"||!use_cpu||layers!=Some(0)||
        host.as_deref()!=Some("127.0.0.1")||
        context.unwrap_or(0)<256||threads.unwrap_or(0)==0{
        return Err("insecure or unsupported fixture configuration".into())
    }
    let password=env::var("KCPP_PASSWORD").map_err(|_|"no host key")?;
    if password.len()!=64||!password.bytes().all(|v|v.is_ascii_hexdigit()){
        return Err("invalid host key".into())
    }
    let port=port.ok_or("missing port")?;
    if port==0 {return Err("port zero".into())}
    Ok((model,port,password))
}
fn response(socket:&mut TcpStream,status:u16,body:&[u8]){
    let reason=if status==200{"OK"}else{"Unauthorized"};
    let header=format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _=socket.write_all(header.as_bytes());
    let _=socket.write_all(body);
}
fn send_event(socket:&mut TcpStream,data:&str)->bool{
    let payload=format!("data: {data}\n\n");
    let chunk=format!("{:X}\r\n",payload.len());
    socket.write_all(chunk.as_bytes()).is_ok() &&
        socket.write_all(payload.as_bytes()).is_ok() &&
        socket.write_all(b"\r\n").is_ok()
}
fn handle(mut socket:TcpStream,password:String){
    let _=socket.set_read_timeout(Some(Duration::from_secs(3)));
    let _=socket.set_write_timeout(Some(Duration::from_secs(3)));
    let mut reader=BufReader::new(&mut socket);
    let mut header=String::new();
    let mut len=0usize;
    let mut first=String::new();
    if reader.read_line(&mut first).is_err() {return}
    while header.len()<16*1024 {
        let mut line=String::new();
        match reader.read_line(&mut line){
            Ok(0)|Err(_)=>return,
            _=>{},
        }
        if line=="\r\n"||line=="\n"{break}
        if line.to_ascii_lowercase().starts_with("content-length:"){
            len=line.split_once(':').and_then(|(_,v)|v.trim().parse().ok())
                .unwrap_or(0);
        }
        header.push_str(&line);
    }
    if len>1024*1024{return}
    let mut raw=vec![0u8;len];
    if reader.read_exact(&mut raw).is_err(){return}
    let valid=header.contains(&format!("Authorization: Bearer {password}"));
    drop(reader);
    if !valid {
        response(&mut socket,401,b"{}");
        return
    }
    if first.starts_with("POST /api/extra/tokencount "){
        response(&mut socket,200,b"{\"tokens\":[1]}");
        return
    }
    if !first.starts_with("POST /v1/chat/completions "){
        response(&mut socket,401,b"{}");
        return
    }
    let Ok(body)=serde_json::from_slice::<serde_json::Value>(&raw) else {
        response(&mut socket,401,b"{}");return
    };
    if body.get("stream").and_then(|v|v.as_bool())!=Some(true) {
        response(&mut socket,401,b"{}");return
    }
    if socket.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    ).is_err(){return}
    let is_tool=body.get("tools").is_some();
    let prompt=body.pointer("/messages/0/content").and_then(|v|v.as_str())
        .unwrap_or("");
    if prompt.contains("P7L_FAKE_BAD_SSE"){
        let _=send_event(&mut socket,
            "{\"choices\":[{\"delta\":{\"content\":7}}]}");
        let _=send_event(&mut socket,"[DONE]");
        let _=socket.write_all(b"0\r\n\r\n");
        return;
    }
    if is_tool {
        let tool_name=if prompt.contains("P7L_FAKE_UNAUTHORIZED_TOOL"){
            "lu_browser_interact"
        }else{
            "lu_browser_read"
        };
        let fragment=json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"id":"call-p7l-1","type":"function",
            "function":{"name":tool_name,
                "arguments":"{\"operation\":\"navigate\",\"url\":\"https://example.org\"}"}
        }]}}]}).to_string();
        if !send_event(&mut socket,&fragment){return}
        let stop=json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}).to_string();
        if !send_event(&mut socket,&stop){return}
    }else{
        for word in ["P7L_","FIXED"]{
            let frame=json!({"choices":[{"delta":{"content":word},
                "finish_reason":serde_json::Value::Null}]}).to_string();
            if !send_event(&mut socket,&frame){return}
            // Enforce a cancellable in-flight stream, not a single buffered
            // completion that cannot exercise the original host flag.
            thread::sleep(Duration::from_millis(60));
        }
        let stop=json!({"choices":[{"delta":{},"finish_reason":"stop"}]}).to_string();
        if !send_event(&mut socket,&stop){return}
    }
    let _=send_event(&mut socket,"[DONE]");
    let _=socket.write_all(b"0\r\n\r\n");
}
fn main(){
    let Ok((_model,port,password))=parse() else{
        std::process::exit(2)
    };
    let Ok(listener)=TcpListener::bind((Ipv4Addr::LOCALHOST,port)) else{
        std::process::exit(3)
    };
    for stream in listener.incoming(){
        let Ok(stream)=stream else{break};
        let key=password.clone();
        thread::spawn(move||handle(stream,key));
    }
}
