//! PERF-01 P0D: deterministic CPU-only original chat retrieval.
//! Not a model executor, filesystem tool, source memory cache or security grant.
use crate::chat_store::ChatStore;
use std::time::{Duration,Instant};

const MAX_QUERY_BYTES:usize=128;
const MAX_SOURCE_SCAN_BYTES:usize=2*1024*1024;
const MAX_SOURCE_ENTRIES:usize=640;
const MAX_MATCHES:usize=8;
const MAX_WALL_MS:u64=300;

#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) enum Route {
    CurrentChatLiteral(String),
    LocalModelStatus,
    ModelRequired(&'static str),
}

/// Exact imperative form only. A quote, directive, model suggestion,
/// attachment or ambiguous semantic search cannot become a CPU answer.
pub(crate) fn classify(prompt:&str,has_images:bool,other_mode:bool)->Route {
    if has_images||other_mode {
        return Route::ModelRequired("NON_TEXT_OR_REVIEWED_MODE");
    }
    let input=prompt.trim();
    if input.len()>512||input.contains('\0')||input.contains("<|")
        ||input.to_ascii_lowercase().contains("[attached text file:") {
        return Route::ModelRequired("COMPLEX_OR_ATTACHED_REQUEST");
    }
    if matches!(input.to_ascii_lowercase().as_str(),
        "show local model status" | "what model is loaded?" |
        "is a model loaded?"){
        return Route::LocalModelStatus;
    }
    let needle=["Search this chat for: ","Find in this conversation: "]
        .iter().find_map(|prefix|input.strip_prefix(prefix));
    let Some(query)=needle else {
        return Route::ModelRequired("OPEN_ENDED_OR_OTHER_CAPABILITY");
    };
    if query.len()<2||query.len()>MAX_QUERY_BYTES||query.trim()!=query||
        query.chars().any(char::is_control)||
        query.contains(char::from(96))||query.contains('<')||query.contains('>') {
        return Route::ModelRequired("LITERAL_QUERY_NOT_ADMISSIBLE");
    }
    Route::CurrentChatLiteral(query.to_owned())
}

fn safe_excerpt(source:&str,needle:&str)->String{
    let at=source.find(needle).unwrap_or(0);
    let char_start=source[..at].chars().count().saturating_sub(36);
    source.chars().skip(char_start).take(220)
        .map(|c|match c{
            '\n'|'\r'|'\t'=>' ',
            '<'|'>'|'['|']'|'*'|'&'|'\\'=>'·',
            x if x==char::from(96)=>'·',
            _=>c
        }).collect()
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) struct CpuSearchResult{
    pub answer:String,
    pub complete:bool,
    pub scanned_entries:usize,
    pub scanned_bytes:usize,
    pub matches:usize,
}

/// Direct ANSF original pages, not an LLM or reconstructed UI history.
/// Active chat is both reader and source scope. Never scan another
/// conversation based on model/text instructions.
pub(crate) fn search_current_chat(
    chats:&ChatStore,chat_id:&str,needle:&str
)->Result<CpuSearchResult,String>{
    if !matches!(classify(&format!("Search this chat for: {needle}"),false,false),
        Route::CurrentChatLiteral(_)) {
        return Err("Invalid bounded CPU literal query".into())
    }
    let start=Instant::now();
    let mut cursor=None;
    let mut scanned_entries=0usize;
    let mut scanned_bytes=0usize;
    let mut matches=0usize;
    let mut lines=Vec::new();
    let mut status="COMPLETE";
    'pages: loop {
        if scanned_entries>=MAX_SOURCE_ENTRIES||scanned_bytes>=MAX_SOURCE_SCAN_BYTES{
            status="PARTIAL_SOURCE_LIMIT";break;
        }
        if start.elapsed()>Duration::from_millis(MAX_WALL_MS){
            status="PARTIAL_TIME_BUDGET";break;
        }
        let page=chats.original_history_page(chat_id,chat_id,cursor,40)?;
        for entry in &page.entries {
            if start.elapsed()>Duration::from_millis(MAX_WALL_MS){
                status="PARTIAL_TIME_BUDGET";break 'pages;
            }
            if scanned_entries>=MAX_SOURCE_ENTRIES{
                status="PARTIAL_SOURCE_LIMIT";break 'pages;
            }
            scanned_entries+=1;
            if !matches!(entry.role.as_str(),"user"|"assistant"){continue}
            // Original source IDs are re-opened from the same ANSF profile.
            // A 4 KiB excerpt is not allowed to imply a complete search.
            let source=chats.original_source_text_page(
                chat_id,&entry.source_id,0,4096
            )?;
            if source.next_offset_chars.is_some(){
                status="PARTIAL_OVERSIZE_SOURCE";
            }
            if scanned_bytes.saturating_add(source.text.len())>MAX_SOURCE_SCAN_BYTES{
                status="PARTIAL_BYTE_LIMIT";break 'pages;
            }
            scanned_bytes+=source.text.len();
            if source.text.contains(needle){
                matches+=1;
                let excerpt=safe_excerpt(&source.text,needle);
                lines.push(format!(
                    "- Original source {} ({}) — {}",
                    entry.source_id,entry.role,excerpt
                ));
                if matches>=MAX_MATCHES{
                    status="PARTIAL_MATCH_LIMIT";break 'pages;
                }
            }
        }
        if !page.has_more {break}
        let Some(next)=page.next_before_id else {
            status="PARTIAL_CURSOR_UNAVAILABLE";break;
        };
        if cursor==Some(next){
            status="PARTIAL_CURSOR_UNAVAILABLE";break;
        }
        cursor=Some(next);
    }
    let complete=status=="COMPLETE";
    let intro=format!(
        "Lu CPU exact-literal lookup · {} · {} matches · {} original entries inspected · {} bytes examined.\n\n",
        status,matches,scanned_entries,scanned_bytes
    );
    let answer=if lines.is_empty(){
        format!("{intro}No visible literal match in the inspected originals. {}",
            if complete{"The bounded original-chat scope was exhausted."}
            else{"This is NOT proof the phrase is absent from older or longer sources."})
    }else{
        let mut text=intro+&lines.join("\n");
        text.push_str("\n\nOriginal ANSF source IDs are historical evidence locators, not tool grants or current-file freshness.");
        if !complete{
            text.push_str("\nSearch was incomplete: additional originals were not ruled out.");
        }
        text
    };
    Ok(CpuSearchResult{answer,complete,scanned_entries,scanned_bytes,matches})
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn strict_cpu_intent_never_steals_general_tasks_or_authority(){
        assert_eq!(classify("Search this chat for: needle",false,false),
            Route::CurrentChatLiteral("needle".into()));
        assert_eq!(classify("Find in this conversation: alpha beta",false,false),
            Route::CurrentChatLiteral("alpha beta".into()));
        assert_eq!(classify("What model is loaded?",false,false),
            Route::LocalModelStatus);
        assert_eq!(classify("Show local model status",false,false),
            Route::LocalModelStatus);
        for prompt in ["Please search this chat for needle",
            "Explain the phrase needle","How to load a model?",
            "Explain local model architecture","Search this chat for: ",
            "Search this chat for: a","Search this chat for: \u{60}exec\u{60}",
            "Search this chat for: <script>","Search this chat for: a\nb",
            "Search this chat for: needle [Attached text file: x]"]{
            assert!(matches!(classify(prompt,false,false),
                Route::ModelRequired(_)), "{prompt}");
        }
        assert!(matches!(classify("Search this chat for: needle",true,false),
            Route::ModelRequired("NON_TEXT_OR_REVIEWED_MODE")));
        assert!(matches!(classify("Search this chat for: needle",false,true),
            Route::ModelRequired("NON_TEXT_OR_REVIEWED_MODE")));
    }
    #[test]
    fn native_original_ansf_lookup_is_chat_scoped_and_model_free(){
        use std::fs;
        let root=std::env::temp_dir().join(format!(
            "lu-perf01-p0d-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let chat=ChatStore::open(&root).unwrap();
        let a=chat.create_chat().unwrap().chat_id;
        let b=chat.create_chat().unwrap().chat_id;
        let x=format!("g-{}","a".repeat(32));
        let y=format!("g-{}","b".repeat(32));
        chat.append_user_with_turn_id(&a,&x,
            "Original alpha-only literal evidence").unwrap();
        chat.append_user_with_turn_id(&b,&y,
            "Other-chat private-only literal").unwrap();
        let answer=search_current_chat(&chat,&a,"alpha-only").unwrap();
        assert!(answer.complete);
        assert_eq!(answer.matches,1);
        assert!(answer.answer.contains("Original source ansf:v1:"));
        assert!(!answer.answer.contains("private-only"));
        let denied=search_current_chat(&chat,&a,"private-only").unwrap();
        assert!(denied.complete);
        assert_eq!(denied.matches,0);
        assert!(search_current_chat(&chat,"c-foreign","alpha-only").is_err());
        drop(chat);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn oversized_original_produces_partial_not_fabricated_absence(){
        use std::fs;
        let root=std::env::temp_dir().join(format!(
            "lu-perf01-oversize-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let chat=ChatStore::open(&root).unwrap();
        let a=chat.create_chat().unwrap().chat_id;
        let t=format!("g-{}","c".repeat(32));
        chat.append_user_with_turn_id(&a,&t,
            &format!("{}masked-at-tail","x".repeat(6_000))).unwrap();
        let query=search_current_chat(&chat,&a,"masked-at-tail").unwrap();
        assert!(!query.complete);
        assert_eq!(query.matches,0);
        assert!(query.answer.contains("NOT proof"));
        drop(chat);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn excerpt_contains_match_without_emitting_raw_markdown_or_html(){
        let input=format!("{}findme<script>alert(1)</script>", "x".repeat(300));
        let excerpt=safe_excerpt(&input,"findme");
        assert!(excerpt.contains("findme"));
        assert!(!excerpt.contains("<script>"));
        assert!(excerpt.len()<800);
    }
}
