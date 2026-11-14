//! M3-S3 local-only original-source semantic embedding worker.
//! The trusted Lu host selects an already bundled and hash-pinned llama-server
//! and a locally selected GGUF embedding model. The worker never receives tool
//! grants, listens outside loopback or sends original ANSF messages to cloud.
//! A worker failure leaves the canonical SQLite book unchanged; vectors are
//! rebuildable derived projections, not historical authority.
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File,OpenOptions},io::{Read,Write,BufRead,BufReader},
    net::{Ipv4Addr,SocketAddr,TcpListener,TcpStream},
    path::PathBuf,process::{Child,Command,Stdio},
    thread,time::{Duration,Instant}
};

pub const MAX_EMBEDDING_INPUT:usize=8*1024;
pub const MAX_EMBEDDING_HTTP:usize=1*1024*1024;

pub struct LocalEmbeddingConfig{
    pub engine:PathBuf,
    pub engine_sha256:[u8;32],
    pub model:PathBuf,
    pub model_sha256:[u8;32],
    pub api_key:String,
    pub dimensions:usize,
    pub threads:u16,
    pub startup_timeout:Duration,
    pub request_timeout:Duration,
}
pub struct LocalEmbeddingWorker{
    child:Child,
    port:u16,
    key:String,
    dims:usize,
    model_id:String,
    timeout:Duration,
    _pinned_model:File,
    #[cfg(windows)]
    job:lu_win_worker_job::Job
}
fn file_digest(path:&PathBuf,expected:[u8;32],gguf:bool)->Result<File,String>{
    if expected.iter().all(|x|*x==0){
        return Err("Pinned engine/model digest is mandatory".into())
    }
    let meta=fs::symlink_metadata(path).map_err(|_|"Pinned engine/model missing")?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()==0||
       meta.len()>16*1024*1024*1024{
        return Err("Pinned model/engine file is not an ordinary bounded file".into())
    }
    let mut options=OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x00000001); // FILE_SHARE_READ
    }
    let mut file=options.open(path).map_err(|_|"Pinned file open failed")?;
    let mut prefix=[0u8;4];
    file.read_exact(&mut prefix).map_err(|_|"Pinned file header unreadable")?;
    if gguf && &prefix != b"GGUF" {
        return Err("Expected a local GGUF embedding model".into())
    }
    use std::io::Seek;
    file.rewind().map_err(|_|"Pinned file seek failed")?;
    let mut hash=Sha256::new();
    let mut bytes=[0u8;1024*1024];
    loop{
        let n=file.read(&mut bytes).map_err(|_|"Pinned file read failed")?;
        if n==0{break}
        hash.update(&bytes[..n]);
    }
    if hash.finalize().as_slice()!=expected{
        return Err("Pinned engine/model SHA-256 mismatch".into())
    }
    file.rewind().map_err(|_|"Pinned file seek failed")?;
    Ok(file)
}
fn strict_vector(v:&Value,dimensions:usize)->Result<Vec<f32>,String>{
    let array=v.get("data").and_then(Value::as_array)
        .ok_or("Embeddings response lacks data array")?;
    if array.len()!=1{return Err("Embedding must return exactly one item".into())}
    let row=&array[0];
    if row.get("index").and_then(Value::as_i64)!=Some(0){
        return Err("Unexpected embedding index".into())
    }
    let xs=row.get("embedding").and_then(Value::as_array)
        .ok_or("Embedding vector missing")?;
    if !(2..=2048).contains(&xs.len()) || (dimensions!=0&&xs.len()!=dimensions){return Err("Embedding dimension mismatch".into())}
    let mut out=Vec::with_capacity(dimensions);
    let mut sq=0f64;
    for x in xs{
        let f=x.as_f64().ok_or("Non-numeric embedding coordinate")?;
        if !f.is_finite()||f<f32::MIN as f64||f>f32::MAX as f64{
            return Err("Nonfinite/out-of-range embedding coordinate".into())
        }
        sq+=f*f;
        out.push(f as f32);
    }
    if sq<=0.0||!sq.is_finite(){
        return Err("Degenerate embedding vector".into())
    }
    Ok(out)
}
fn decode_chunked(body:&[u8])->Result<Vec<u8>,String>{
    let mut at=0usize;
    let mut output=Vec::new();
    loop{
        let end=body[at..].windows(2).position(|s|s==b"\r\n")
            .ok_or("Invalid chunked embedding response")?+at;
        let len_str=std::str::from_utf8(&body[at..end])
            .map_err(|_|"Invalid chunk length encoding")?;
        let size=usize::from_str_radix(len_str.split(';').next()
            .ok_or("Missing chunk size")?,16)
            .map_err(|_|"Invalid chunk length")?;
        at=end+2;
        if size==0{return Ok(output)}
        if size>MAX_EMBEDDING_HTTP||output.len()+size>MAX_EMBEDDING_HTTP||
           at+size+2>body.len()||&body[at+size..at+size+2]!=b"\r\n"{
            return Err("Oversized/truncated chunked embedding response".into())
        }
        output.extend_from_slice(&body[at..at+size]);
        at+=size+2;
    }
}
fn parse_http(raw:&[u8],dims:usize)->Result<Vec<f32>,String>{
    let split=raw.windows(4).position(|x|x==b"\r\n\r\n")
        .ok_or("Missing embedding HTTP headers")?;
    let header=std::str::from_utf8(&raw[..split])
        .map_err(|_|"Invalid HTTP header bytes")?;
    let status=header.lines().next().unwrap_or("");
    if !status.starts_with("HTTP/1.1 200 ")&&
       !status.starts_with("HTTP/1.0 200 "){
        return Err("Local embedding service returned non-200 response".into())
    }
    let chunked=header.lines().skip(1).any(|x|
        x.to_ascii_lowercase().starts_with("transfer-encoding:")&&
        x.to_ascii_lowercase().contains("chunked"));
    let body=&raw[split+4..];
    let data=if chunked{decode_chunked(body)?}else{
        if let Some(line)=header.lines().skip(1).find(|x|
            x.to_ascii_lowercase().starts_with("content-length:")){
            let num=line.split_once(':').ok_or("Malformed content length")?
                .1.trim().parse::<usize>().map_err(|_|"Bad content length")?;
            if num!=body.len()||num>MAX_EMBEDDING_HTTP{
                return Err("Invalid embedding response length".into())
            }
        }
        body.to_vec()
    };
    let value:Value=serde_json::from_slice(&data)
        .map_err(|_|"Invalid embedding JSON response")?;
    strict_vector(&value,dims)
}
fn request_embedding(
    port:u16,key:&str,text:&str,dims:usize,timeout:Duration
)->Result<Vec<f32>,String>{
    if text.trim().is_empty()||text.len()>MAX_EMBEDDING_INPUT{
        return Err("Embedding passage exceeds bounded input".into())
    }
    let addr=SocketAddr::from((Ipv4Addr::LOCALHOST,port));
    let mut socket=TcpStream::connect_timeout(&addr,Duration::from_millis(800))
        .map_err(|_|"Trusted local embedding worker is unavailable")?;
    socket.set_read_timeout(Some(timeout)).map_err(|_|"Embedding socket timeout")?;
    socket.set_write_timeout(Some(timeout)).map_err(|_|"Embedding socket timeout")?;
    let body=json!({"input":text,"encoding_format":"float"}).to_string();
    let header=format!(
        "POST /v1/embeddings HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        key,body.len()
    );
    socket.write_all(header.as_bytes()).map_err(|_|"Embedding request write failed")?;
    socket.write_all(body.as_bytes()).map_err(|_|"Embedding request write failed")?;
    let mut bytes=Vec::new();
    socket.take((MAX_EMBEDDING_HTTP+8192) as u64+1)
        .read_to_end(&mut bytes).map_err(|_|"Embedding response read failed")?;
    if bytes.len()>MAX_EMBEDDING_HTTP+8192{
        return Err("Embedding response exceeded byte limit".into())
    }
    parse_http(&bytes,dims)
}
fn safe_key(key:&str)->bool{
    (32..=128).contains(&key.len())&&
    key.bytes().all(|b|b.is_ascii_alphanumeric())
}
impl LocalEmbeddingWorker{
    pub fn start(config:LocalEmbeddingConfig)->Result<Self,String>{
        if (config.dimensions!=0&&!(2..=2048).contains(&config.dimensions))||config.threads==0||
           config.threads>32||!safe_key(&config.api_key)||
           config.startup_timeout.is_zero()||
           config.startup_timeout>Duration::from_secs(300)||
           config.request_timeout.is_zero()||
           config.request_timeout>Duration::from_secs(120){
            return Err("Invalid supervised embedding configuration".into())
        }
        // Engine and model provenance comes from trusted host manifest and OS
        // selected file respectively; NEVER from a remembered model instruction.
        let engine_pin=file_digest(&config.engine,config.engine_sha256,false)?;
        let model_pin=file_digest(&config.model,config.model_sha256,true)?;
        drop(engine_pin);
        let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0))
            .map_err(|_|"Cannot reserve local embedding port")?;
        let port=listener.local_addr().map_err(|_|"Cannot read local port")?.port();
        drop(listener);
        let mut cmd=Command::new(&config.engine);
        cmd.arg("--model").arg(&config.model)
            .arg("--embedding").arg("--pooling").arg("mean")
            .arg("--host").arg("127.0.0.1").arg("--port").arg(port.to_string())
            .arg("--api-key").arg(&config.api_key)
            .arg("--threads").arg(config.threads.to_string())
            .arg("--parallel").arg("1").arg("--n-gpu-layers").arg("0")
            .arg("--no-warmup")
            .current_dir(config.engine.parent()
                .ok_or("Invalid embedding engine location")?)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        for (key,_) in std::env::vars_os(){
            let name=key.to_string_lossy().to_ascii_uppercase();
            if name.starts_with("LLAMA_ARG_")||name.starts_with("GGML_")||
               name.starts_with("DYLD_")||
               name=="LD_PRELOAD"||name=="LD_AUDIT"{
                cmd.env_remove(key);
            }
        }
        #[cfg(target_os="linux")]
        cmd.env("LD_LIBRARY_PATH",config.engine.parent()
            .ok_or("Invalid engine directory")?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // hidden, assigned to Job immediately
        }
        #[cfg(windows)]
        let job=lu_win_worker_job::Job::new().map_err(|_|"Embedding process Job failed")?;
        let mut child=cmd.spawn().map_err(|_|"Pinned local embedding spawn failed")?;
        #[cfg(windows)]
        if let Err(_)=job.assign(&child){
            let _=child.kill();let _=child.wait();
            return Err("Embedding child Job assignment failed".into())
        }
        // Avoid double-hashing the pinned hash: vector identity must equal the
        // original SHA-256 of the GGUF model, so the model ID is stable.
        let model_id=format!("ggufsha256-{}",config.model_sha256.iter()
            .map(|b|format!("{:02x}",b)).collect::<String>());
        let mut worker=Self{
            child,port,key:config.api_key,dims:config.dimensions,
            model_id,timeout:config.request_timeout,_pinned_model:model_pin,
            #[cfg(windows)]job
        };
        let deadline=Instant::now()+config.startup_timeout;
        loop{
            if worker.child.try_wait().map_err(|_|"Embedding worker status failed")?.is_some(){
                return Err("Embedding model worker exited during startup".into())
            }
            if Instant::now()>=deadline{
                return Err("Embedding worker timed out loading local GGUF".into())
            }
            if let Ok(mut sock)=TcpStream::connect_timeout(
                &SocketAddr::from((Ipv4Addr::LOCALHOST,port)),
                Duration::from_millis(200)){
                sock.set_read_timeout(Some(Duration::from_millis(500))).ok();
                let health=format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",worker.key);
                if sock.write_all(health.as_bytes()).is_ok(){
                    let mut r=BufReader::new(sock);let mut status=String::new();
                    if r.read_line(&mut status).is_ok()&&
                       status.starts_with("HTTP/1.1 200 "){
                        // A healthy chat server is not proof of a valid
                        // *embedding* model. Require a real vector first.
                        worker.embed("Local semantic retrieval readiness")?;
                        return Ok(worker)
                    }
                }
            }
            thread::sleep(Duration::from_millis(150));
        }
    }
    pub fn model_id(&self)->&str{&self.model_id}
    pub fn embed(&mut self,text:&str)->Result<Vec<f32>,String>{
        if self.child.try_wait().map_err(|_|"Embedding worker status unavailable")?.is_some(){
            return Err("Embedding worker has exited".into())
        }
        let vector=request_embedding(self.port,&self.key,text,self.dims,self.timeout)?;
        if self.dims==0{self.dims=vector.len()}
        Ok(vector)
    }
}
impl Drop for LocalEmbeddingWorker{
    fn drop(&mut self){
        let _=self.child.kill();
        let _=self.child.wait();
        #[cfg(windows)]
        let _=&self.job;
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn accepts_only_one_valid_finite_vector_and_rejects_hostile_shapes(){
        let good=json!({"data":[{"index":0,"embedding":[0.5,-0.75,1.0]}]});
        assert_eq!(strict_vector(&good,3).unwrap(),vec![0.5,-0.75,1.0]);
        assert!(strict_vector(&good,2).is_err());
        assert!(strict_vector(&json!({"data":[]}),3).is_err());
        assert!(strict_vector(&json!({"data":[{"index":0,"embedding":[0,0,0]}]}),3).is_err());
        assert!(strict_vector(&json!({"data":[{"index":0,"embedding":[1,2,3]},{"index":1,"embedding":[0,0,1]}]}),3).is_err());
    }
    #[test]
    fn accepts_embedded_json_body_and_http_chunked_response_without_unbounded_reads(){
        let body=json!({"data":[{"index":0,"embedding":[0.1,0.3]}]}).to_string();
        let raw=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",body.len(),body);
        assert_eq!(parse_http(raw.as_bytes(),2).unwrap().len(),2);
        let chunk=format!("{:x}\r\n{}\r\n0\r\n\r\n",body.len(),body);
        let raw=format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{}",chunk);
        assert_eq!(parse_http(raw.as_bytes(),2).unwrap().len(),2);
        assert!(parse_http(b"HTTP/1.1 401 Unauthorized\r\n\r\n{}",2).is_err());
        assert!(parse_http(b"HTTP/1.1 200 OK\r\nContent-Length: 2000000\r\n\r\n{}",2).is_err());
    }
    #[test]
    fn real_loopback_request_uses_private_api_key_and_never_an_external_address(){
        let listener=TcpListener::bind((Ipv4Addr::LOCALHOST,0)).unwrap();
        let port=listener.local_addr().unwrap().port();
        let serving=std::thread::spawn(move||{
            let (mut stream,_)=listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut reader=BufReader::new(&mut stream);
            let mut headers=String::new();
            loop{
                let mut line=String::new();
                reader.read_line(&mut line).unwrap();
                if line=="\r\n"{break}
                headers.push_str(&line);
            }
            assert!(headers.contains("POST /v1/embeddings HTTP/1.1"));
            assert!(headers.contains("Authorization: Bearer fixedsecret0123456789fixedsecret12"));
            let length=headers.lines().find(|s|s.starts_with("Content-Length:"))
                .unwrap().split_once(':').unwrap().1.trim().parse::<usize>().unwrap();
            let mut buf=vec![0;length];
            reader.read_exact(&mut buf).unwrap();
            assert!(std::str::from_utf8(&buf).unwrap().contains("worker restart"));
            drop(reader);
            let body=json!({"data":[{"index":0,"embedding":[0.1,0.2]}]}).to_string();
            stream.write_all(format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body
            ).as_bytes()).unwrap();
        });
        let v=request_embedding(port,"fixedsecret0123456789fixedsecret12",
            "worker restart",2,Duration::from_secs(2)).unwrap();
        assert_eq!(v.len(),2);
        serving.join().unwrap();
    }
}
