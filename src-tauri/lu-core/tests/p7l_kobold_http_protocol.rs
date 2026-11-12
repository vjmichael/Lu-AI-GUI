//! P7L native Rust real-socket, synthetic OpenAI chat/tool SSE matrix.
//! These tests use actual loopback TCP and Lu's original bounded HTTP parser.
//! They deliberately DO NOT claim the synthetic responder is a KoboldCpp
//! binary or that a GPU kernel, GGUF or OS process was exercised.
use lu_core::multimodal_http::{
    generate_chat_stream_body,generate_browser_tool_stream_body
};
use std::{
    collections::BTreeSet,
    io::{BufRead,BufReader,Read,Write},
    net::{Ipv4Addr,TcpListener},
    sync::{Arc,atomic::{AtomicBool,Ordering}},
    thread,
    time::Duration,
};

const KEY:&str="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn socket_fixture(
    frames:Vec<String>, expected_status:u16,
)->(u16,thread::JoinHandle<()>){
    let server=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();
    let port=server.local_addr().unwrap().port();
    let handle=thread::spawn(move||{
        let (mut stream,_)=server.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut reader=BufReader::new(&mut stream);
        let mut headers=String::new();
        let mut first=true;
        let mut content=0usize;
        loop{
            let mut line=String::new();
            let n=reader.read_line(&mut line).unwrap();
            assert!(n>0,"request truncated");
            if first{
                assert!(line.starts_with("POST /v1/chat/completions HTTP/1.1"));
                first=false;
            }
            if line=="\r\n"{break}
            headers.push_str(&line);
            if line.to_ascii_lowercase().starts_with("content-length:"){
                content=line.trim().split_once(':').unwrap().1.trim()
                    .parse().unwrap();
            }
            assert!(headers.len()<16*1024);
        }
        assert!(headers.contains(&format!("Authorization: Bearer {KEY}")));
        assert!(content>0&&content<48*1024*1024);
        let mut data=vec![0u8;content];
        reader.read_exact(&mut data).unwrap();
        let body=String::from_utf8(data).unwrap();
        assert!(body.contains("\"stream\":true")||
            body.contains("\"stream\": true"));
        assert!(body.contains("\"messages\""));
        drop(reader);
        if expected_status!=200{
            let response=format!(
                "HTTP/1.1 {expected_status} Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _=stream.write_all(response.as_bytes());
            return;
        }
        stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
        ).unwrap();
        for bytes in frames{
            let block=bytes.as_bytes();
            let chunk=format!("{:X}\r\n",block.len());
            if stream.write_all(chunk.as_bytes()).is_err(){break}
            if stream.write_all(block).is_err(){break}
            if stream.write_all(b"\r\n").is_err(){break}
        }
        let _=stream.write_all(b"0\r\n\r\n");
    });
    (port,handle)
}
fn sse(data:&str)->String{format!("data: {data}\n\n")}
fn body()->&'static str {
    r#"{"messages":[{"role":"user","content":"fixed synthetic prompt"}],"temperature":0,"max_tokens":16,"stream":true}"#
}
fn text_delta(value:&str)->String{
    sse(&format!(
        r#"{{"choices":[{{"index":0,"delta":{{"content":"{value}"}},"finish_reason":null}}]}}"#
    ))
}
#[test]
fn exact_openai_text_stream_handles_fragments_across_http_chunks(){
    let first=text_delta("alpha");
    let divided=first.len()/2;
    let (port,server)=socket_fixture(vec![
        first[..divided].into(),first[divided..].into(),
        text_delta("beta"),sse(r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#),
        sse("[DONE]"),
    ],200);
    let mut result=String::new();
    generate_chat_stream_body(port,KEY,body(),Duration::from_secs(3),
        ||false,|part|{result.push_str(&part);Ok(())}).unwrap();
    assert_eq!(result,"alphabeta");
    server.join().unwrap();
}
#[test]
fn streamed_browser_call_requires_original_function_name_and_valid_json(){
    let allowed=BTreeSet::from(["lu_browser_read".to_string()]);
    let (port,server)=socket_fixture(vec![
        sse(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"lu_browser_read","arguments":"{\\"operation\\":\\"navigate\\","}}]}}]}"#),
        sse(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":",\\"url\\":\\"https://example.org\\"}"}}]}}]}"#),
        sse(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#),
        sse("[DONE]"),
    ],200);
    let calls=generate_browser_tool_stream_body(
        port,KEY,body(),Duration::from_secs(3),||false,allowed
    ).unwrap();
    assert_eq!(calls.len(),1);
    assert_eq!(calls[0].tool_name,"lu_browser_read");
    assert_eq!(calls[0].arguments_json,
        r#"{"operation":"navigate","url":"https://example.org"}"#);
    server.join().unwrap();
}
#[test]
fn rejects_unadvertised_tool_even_from_successful_http(){
    let (port,server)=socket_fixture(vec![
        sse(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-2","type":"function","function":{"name":"lu_browser_interact","arguments":"{}"}}]}}]}"#),
        sse(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#),
        sse("[DONE]"),
    ],200);
    let got=generate_browser_tool_stream_body(
        port,KEY,body(),Duration::from_secs(3),||false,
        BTreeSet::from(["lu_browser_read".to_string()])
    );
    assert!(got.is_err());
    server.join().unwrap();
}
#[test]
fn error_frame_and_http_401_never_complete_as_text(){
    let (port,server)=socket_fixture(vec![],401);
    assert!(generate_chat_stream_body(
        port,KEY,body(),Duration::from_secs(3),||false,|_|Ok(())
    ).is_err());
    server.join().unwrap();
    let (port,server)=socket_fixture(vec![
        sse(r#"{"error":{"message":"backend failed"}}"#),sse("[DONE]")
    ],200);
    assert!(generate_chat_stream_body(
        port,KEY,body(),Duration::from_secs(3),||false,|_|Ok(())
    ).is_err());
    server.join().unwrap();
}
#[test]
fn consumer_cancellation_interrupts_live_original_sse_reader(){
    let (port,server)=socket_fixture(vec![
        text_delta("one"),text_delta("two"),sse("[DONE]")
    ],200);
    let interrupted=Arc::new(AtomicBool::new(false));
    let check=Arc::clone(&interrupted);
    let trigger=Arc::clone(&interrupted);
    let response=generate_chat_stream_body(
        port,KEY,body(),Duration::from_secs(3),
        ||check.load(Ordering::Acquire),
        |_part|{trigger.store(true,Ordering::Release);Ok(())}
    );
    assert!(response.is_err());
    server.join().unwrap();
}
