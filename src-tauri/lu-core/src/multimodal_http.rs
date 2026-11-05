//! Host-controlled, authenticated loopback transport for llama.cpp's
//! OpenAI-compatible multimodal `/v1/chat/completions` stream. Not Lu IPC.
//! Both the images and the prompt must have been reconstructed from the
//! trusted host-opened saved conversation file before this entrypoint.
use crate::multimodal_saved_turn::{
    build_llama_chat_request, parse_chat_sse_frame, ChatEvent, SavedMultimodalTurn,
};
use std::{
    io::{self, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    time::{Duration, Instant},
};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_EVENT_BYTES: usize = 64 * 1024;
const MAX_STREAM_BYTES: usize = 16 * 1024 * 1024;

fn check(cancelled: &impl Fn() -> bool, deadline: Instant) -> Result<(), String> {
    if cancelled() {
        return Err("multimodal generation cancelled".into());
    }
    if Instant::now() >= deadline {
        return Err("multimodal generation timed out".into());
    }
    Ok(())
}

fn read_exact_bounded(
    reader: &mut impl Read,
    bytes: &mut [u8],
    cancelled: &impl Fn() -> bool,
    deadline: Instant,
) -> Result<(), String> {
    let mut at = 0;
    while at < bytes.len() {
        check(cancelled, deadline)?;
        match reader.read(&mut bytes[at..]) {
            Ok(0) => return Err("llama server truncated multimodal response".into()),
            Ok(n) => at += n,
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("multimodal response read failed".into()),
        }
    }
    Ok(())
}

fn read_line_bounded(
    reader: &mut impl Read,
    max: usize,
    cancelled: &impl Fn() -> bool,
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        check(cancelled, deadline)?;
        if out.len() >= max {
            return Err("multimodal HTTP line exceeds size limit".into());
        }
        let mut b = [0u8; 1];
        match reader.read(&mut b) {
            Ok(0) => return Err("llama server closed multimodal stream".into()),
            Ok(_) => {
                out.push(b[0]);
                if b[0] == b'\n' {
                    return Ok(out);
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("multimodal HTTP line read failed".into()),
        }
    }
}

fn next_frame(events: &mut Vec<u8>) -> Option<Vec<u8>> {
    let lf = events.windows(2).position(|w| w == b"\n\n").map(|p| (p, 2));
    let crlf = events
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| (p, 4));
    let (at, delimiter) = match (lf, crlf) {
        (Some(a), Some(b)) => {
            if a.0 < b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        _ => return None,
    };
    Some(events.drain(..at + delimiter).collect())
}

fn write_checked(
    socket: &mut TcpStream,
    bytes: &[u8],
    cancelled: &impl Fn() -> bool,
    deadline: Instant,
) -> Result<(), String> {
    for block in bytes.chunks(64 * 1024) {
        let mut at = 0;
        while at < block.len() {
            check(cancelled, deadline)?;
            match socket.write(&block[at..]) {
                Ok(0) => return Err("llama server closed multimodal upload".into()),
                Ok(n) => at += n,
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err("multimodal request upload failed".into()),
            }
        }
    }
    Ok(())
}

pub fn generate_image_chat_stream(
    port: u16,
    api_key: &str,
    turn: &SavedMultimodalTurn,
    max_new_tokens: u32,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
    on_text: impl FnMut(String) -> Result<(), String>,
) -> Result<(), String> {
    let body = build_llama_chat_request(turn, max_new_tokens)
        .map_err(|_| "invalid admitted multimodal request".to_string())?;
    generate_chat_stream_body(port, api_key, &body, timeout, cancelled, on_text)
}

/// The JSON format here is strictly the external llama.cpp HTTP contract.
/// Host / worker / model authority remains carried through Lu's Protobuf IPC.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum FrameDisposition {Continue,Done}
/// Shared authenticated transport and chunk framing. The handler validates
/// the semantic SSE payload; no arbitrary function text is ever executed.
fn stream_chat_frames(
    port: u16,
    api_key: &str,
    body: &str,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
    mut on_frame: impl FnMut(&[u8]) -> Result<FrameDisposition,String>,
) -> Result<(),String> {
    if port == 0
        || api_key.len() != 64
        || !api_key.bytes().all(|b| b.is_ascii_hexdigit())
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
        || body.is_empty()
        || body.len() > 48 * 1024 * 1024
    {
        return Err("invalid trusted chat endpoint settings".into());
    }
    let deadline = Instant::now() + timeout;
    check(&cancelled, deadline)?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|_| "Lu multimodal engine unavailable".to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|_| "engine read timeout invalid")?;
    socket
        .set_write_timeout(Some(Duration::from_millis(200)))
        .map_err(|_| "engine write timeout invalid")?;
    let headers = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {api_key}\r\nContent-Type: application/json\r\nAccept: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    write_checked(&mut socket, headers.as_bytes(), &cancelled, deadline)?;
    write_checked(&mut socket, body.as_bytes(), &cancelled, deadline)?;
    let mut reader = BufReader::new(socket);
    let status = read_line_bounded(&mut reader, MAX_HEADER_BYTES, &cancelled, deadline)?;
    if !status.starts_with(b"HTTP/1.1 200 ") && !status.starts_with(b"HTTP/1.0 200 ") {
        // No engine response body, request content, image bytes, or credentials
        // cross this error boundary. HTTP status alone identifies the class.
        let code = std::str::from_utf8(&status)
            .ok()
            .and_then(|line| line.split_whitespace().nth(1))
            .filter(|v| v.len() == 3 && v.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or("unknown");
        return Err(format!("llama.cpp chat request rejected (HTTP {code}); verify matching GGUF/MMProj, chat-template support and engine diagnostics"));
    }
    let mut header_total = status.len();
    let mut chunked = false;
    let mut event_stream = false;
    loop {
        let line = read_line_bounded(&mut reader, MAX_HEADER_BYTES, &cancelled, deadline)?;
        header_total = header_total.saturating_add(line.len());
        if header_total > MAX_HEADER_BYTES {
            return Err("oversized multimodal response headers".into());
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        let header = String::from_utf8_lossy(&line).to_ascii_lowercase();
        if header.starts_with("transfer-encoding:") && header.contains("chunked") {
            chunked = true;
        }
        if header.starts_with("content-type:") && header.contains("text/event-stream") {
            event_stream = true;
        }
    }
    if !chunked || !event_stream {
        return Err("multimodal backend returned unsupported streaming encoding".into());
    }
    let mut events: Vec<u8> = Vec::new();
    let mut streamed = 0usize;
    let mut complete = false;
    while !complete {
        let size = read_line_bounded(&mut reader, 128, &cancelled, deadline)?;
        let size = String::from_utf8_lossy(&size);
        let raw = size.trim().split(';').next().unwrap_or("");
        let length =
            usize::from_str_radix(raw, 16).map_err(|_| "invalid multimodal chunk framing")?;
        if length == 0 {
            break;
        }
        streamed = streamed.saturating_add(length);
        if length > MAX_EVENT_BYTES || streamed > MAX_STREAM_BYTES {
            return Err("multimodal response exceeds size limit".into());
        }
        let mut chunk = vec![0u8; length];
        read_exact_bounded(&mut reader, &mut chunk, &cancelled, deadline)?;
        let mut terminator = [0u8; 2];
        read_exact_bounded(&mut reader, &mut terminator, &cancelled, deadline)?;
        if terminator != *b"\r\n" {
            return Err("invalid multimodal chunk terminator".into());
        }
        events.extend_from_slice(&chunk);
        if events.len() > MAX_EVENT_BYTES {
            return Err("oversized multimodal SSE event".into());
        }
        while let Some(frame) = next_frame(&mut events) {
            if on_frame(&frame)? == FrameDisposition::Done {
                complete = true;
                break;
            }
        }
    }
    check(&cancelled, deadline)?;
    if !complete {
        return Err("multimodal engine stream did not complete".into());
    }
    Ok(())
}

/// Original text-only generation path: strictly preserve its nonempty reply
/// contract. Tool frames are not interpreted as model prose or executable
/// instructions in this path.
pub fn generate_chat_stream_body(
    port:u16,
    api_key:&str,
    body:&str,
    timeout:Duration,
    cancelled:impl Fn()->bool,
    mut on_text:impl FnMut(String)->Result<(),String>,
)->Result<(),String>{
    let mut emitted=0usize;
    stream_chat_frames(port,api_key,body,timeout,cancelled,|frame|{
        match parse_chat_sse_frame(frame).map_err(|_|"invalid multimodal chat event")?{
            ChatEvent::Text(text)=>{
                emitted=emitted.saturating_add(text.len());
                if emitted>MAX_STREAM_BYTES{return Err("multimodal text output limit exceeded".into());}
                on_text(text)?;
                Ok(FrameDisposition::Continue)
            }
            ChatEvent::Continue=>Ok(FrameDisposition::Continue),
            ChatEvent::Done=>Ok(FrameDisposition::Done),
        }
    })?;
    if emitted==0{return Err("multimodal engine did not finish with nonempty text".into());}
    Ok(())
}

/// Structured function calls are collected in isolation from the text stream.
pub fn generate_browser_tool_stream_body(
    port:u16,
    api_key:&str,
    body:&str,
    timeout:Duration,
    cancelled:impl Fn()->bool,
    allowed_names:std::collections::BTreeSet<String>,
)->Result<Vec<crate::browser_tool_stream::CompletedToolCall>,String>{
    use crate::browser_tool_stream::{
        parse_tool_sse_frame,BrowserToolCallCollector,ToolStreamEvent,
    };
    if allowed_names.is_empty()||allowed_names.len()>2
        ||allowed_names.iter().any(|x|x!="lu_browser_read"){
        return Err("browser tool stream has no approved read capability".into());
    }
    let mut collector=BrowserToolCallCollector::new(allowed_names);
    let mut completed=None;
    stream_chat_frames(port,api_key,body,timeout,cancelled,|frame|{
        let event=parse_tool_sse_frame(frame)
            .map_err(|_|"invalid browser tool SSE event")?;
        let done=matches!(event,ToolStreamEvent::Done);
        let stopped=matches!(event,ToolStreamEvent::Stop);
        if let Some(calls)=collector.absorb(event)
            .map_err(|_|"invalid or unadvertised browser tool call")?{
            completed=Some(calls);
        }
        if stopped {
            // Only a clean model stop, not an unfinished stream, signals
            // no further tool calls. Host checks evidence before finalizing.
            if completed.is_some(){return Err("conflicting browser stream endings".into());}
            completed=Some(Vec::new());
        }
        if done {
            if completed.is_none(){
                return Err("browser-enabled turn finished without approved tool calls".into());
            }
            return Ok(FrameDisposition::Done);
        }
        Ok(FrameDisposition::Continue)
    })?;
    completed.ok_or_else(||"browser tool stream did not return approved calls".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
    use std::io::{BufRead, Cursor};
    use std::net::TcpListener;
    use std::thread;
    fn turn() -> SavedMultimodalTurn {
        let mut image = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([255, 2, 3])))
            .write_to(&mut image, ImageFormat::Png)
            .unwrap();
        let dataurl = format!("data:image/png;base64,{}", STANDARD.encode(image.get_ref()));
        let image = crate::multimodal_saved_turn::admit_data_url(&dataurl).unwrap();
        SavedMultimodalTurn {
            prompt: "What color is this?".into(),
            images: vec![image],
        }
    }
    fn response(parts: &[&[u8]]) -> Vec<u8> {
        let mut body=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: text/event-stream\r\n\r\n".to_vec();
        for p in parts {
            body.extend_from_slice(format!("{:x}\r\n", p.len()).as_bytes());
            body.extend_from_slice(p);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"0\r\n\r\n");
        body
    }
    fn fixture(response: Vec<u8>, check_body: bool) -> (u16, thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let (mut sock, _) = l.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut reader = BufReader::new(&mut sock);
            let mut headers = String::new();
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if line.starts_with("POST ") {
                    assert_eq!(line, "POST /v1/chat/completions HTTP/1.1\r\n");
                }
                if let Some(x) = line.to_lowercase().strip_prefix("content-length:") {
                    len = x.trim().parse::<usize>().unwrap();
                }
                headers.push_str(&line);
            }
            assert!(headers.contains("Authorization: Bearer "));
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).unwrap();
            if check_body {
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(value["messages"][0]["content"][1]["type"], "image_url");
            }
            drop(reader);
            sock.write_all(&response).unwrap();
        });
        (port, handle)
    }
    #[test]
    fn browser_tool_stream_accepts_clean_stop_but_not_bare_done() {
        let natural=b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        let (port,handle)=fixture(response(&[natural]),false);
        let tools=generate_browser_tool_stream_body(port,&"ab".repeat(32),
            "{}",Duration::from_secs(10),||false,
            ["lu_browser_read".into()].into());
        handle.join().unwrap();
        assert!(tools.unwrap().is_empty());
        let bare=b"data: [DONE]\n\n";
        let (port,handle)=fixture(response(&[bare]),false);
        let result=generate_browser_tool_stream_body(port,&"ab".repeat(32),
            "{}",Duration::from_secs(10),||false,
            ["lu_browser_read".into()].into());
        handle.join().unwrap();
        assert!(result.is_err());
    }
    #[test]
    fn streamed_image_request_reassembles_arbitrary_sse_chunks() {
        let stream=b"data: {\"choices\":[{\"delta\":{\"content\":\"A red\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" square\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n";
        let a = &stream[..17];
        let b = &stream[17..89];
        let c = &stream[89..];
        let (port, handle) = fixture(response(&[a, b, c]), true);
        let mut out = String::new();
        let result = generate_image_chat_stream(
            port,
            &"ab".repeat(32),
            &turn(),
            128,
            Duration::from_secs(10),
            || false,
            |text| {
                out.push_str(&text);
                Ok(())
            },
        );
        handle.join().unwrap();
        result.unwrap();
        assert_eq!(out, "A red square");
    }
    #[test]
    fn missing_text_is_a_failure_not_success() {
        let stream = b"data: [DONE]\n\n";
        let (port, handle) = fixture(response(&[stream]), false);
        let result = generate_image_chat_stream(
            port,
            &"ab".repeat(32),
            &turn(),
            128,
            Duration::from_secs(10),
            || false,
            |_| Ok(()),
        );
        handle.join().unwrap();
        assert!(result.is_err());
    }
    #[test]
    fn rejects_legacy_text_completion_stream() {
        let stream = b"data: {\"content\":\"hello\",\"stop\":true}\n\n";
        let (port, handle) = fixture(response(&[stream]), false);
        let result = generate_image_chat_stream(
            port,
            &"ab".repeat(32),
            &turn(),
            128,
            Duration::from_secs(10),
            || false,
            |_| Ok(()),
        );
        handle.join().unwrap();
        assert!(result.is_err());
    }
    #[test]
    fn rejects_wrong_api_key_before_network_use() {
        assert!(generate_image_chat_stream(
            1234,
            "bad",
            &turn(),
            128,
            Duration::from_secs(1),
            || false,
            |_| Ok(())
        )
        .is_err());
    }
    #[test]
    fn rejects_cancelled_before_opening_socket() {
        assert!(generate_image_chat_stream(
            1234,
            &"ab".repeat(32),
            &turn(),
            128,
            Duration::from_secs(1),
            || true,
            |_| Ok(())
        )
        .is_err());
    }
    #[test]
    fn rejection_exposes_only_status_not_server_supplied_text() {
        let stream=b"HTTP/1.1 400 Bad Request\r\nContent-Length: 44\r\n\r\nsecret-image-content-shall-not-be-returned".to_vec();
        let (port, handle) = fixture(stream, true);
        let err = generate_image_chat_stream(
            port,
            &"ab".repeat(32),
            &turn(),
            32,
            Duration::from_secs(5),
            || false,
            |_| Ok(()),
        )
        .unwrap_err();
        handle.join().unwrap();
        assert!(err.contains("HTTP 400"));
        assert!(!err.contains("secret-image-content"));
    }
    #[test]
    fn actual_authenticated_sse_transport_delivers_fragmented_browser_tool_calls(){
        let first=serde_json::json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"id":"call-browser-1","type":"function",
            "function":{"name":"lu_browser_read",
                "arguments":"{\"operation\":\"navigate\","}
        }]}}]});
        let second=serde_json::json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"function":{"arguments":"\"url\":\"https://example.com\"}"}
        }]}}]});
        let end=serde_json::json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]});
        let stream=format!(
            "data: {}\\n\\ndata: {}\\n\\ndata: {}\\n\\ndata: [DONE]\\n\\n",
            first,second,end
        ).replace("\\n","\n");
        let parts=[&stream.as_bytes()[..31],&stream.as_bytes()[31..]];
        let (port,handle)=fixture(response(&parts),false);
        let allowed=std::collections::BTreeSet::from(["lu_browser_read".into()]);
        let calls=generate_browser_tool_stream_body(
            port,&"ab".repeat(32),r#"{"stream":true,"tools":[{"type":"function"}]}"#,
            Duration::from_secs(5),||false,allowed
        ).unwrap();
        handle.join().unwrap();
        assert_eq!(calls.len(),1);
        assert_eq!(calls[0].tool_name,"lu_browser_read");
        assert_eq!(calls[0].arguments_json,
            r#"{"operation":"navigate","url":"https://example.com"}"#);
    }
    #[test]
    fn browser_tool_stream_requires_final_done_frame(){
        let first=serde_json::json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"id":"call-browser-1","type":"function",
            "function":{"name":"lu_browser_read",
                "arguments":"{\"operation\":\"read\"}"}
        }]}}]});
        let end=serde_json::json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]});
        let stream=format!("data: {}\\n\\ndata: {}\\n\\n",first,end)
            .replace("\\n","\n");
        let (port,handle)=fixture(response(&[stream.as_bytes()]),false);
        let result=generate_browser_tool_stream_body(
            port,&"ab".repeat(32),"{}",Duration::from_secs(5),||false,
            std::collections::BTreeSet::from(["lu_browser_read".into()])
        );
        handle.join().unwrap();
        assert!(result.is_err());
    }
    #[test]
    fn text_generation_uses_chat_completions_not_raw_completion() {
        let stream = b"data: {\"choices\":[{\"delta\":{\"content\":\"Ready\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n";
        let (port, handle) = fixture(response(&[stream]), false);
        let body =
            crate::multimodal_saved_turn::build_structured_chat_request("Ping", &[], &[], 20)
                .unwrap();
        let mut text = String::new();
        generate_chat_stream_body(
            port,
            &"ab".repeat(32),
            &body,
            Duration::from_secs(5),
            || false,
            |piece| {
                text.push_str(&piece);
                Ok(())
            },
        )
        .unwrap();
        handle.join().unwrap();
        assert_eq!(text, "Ready");
    }
}
