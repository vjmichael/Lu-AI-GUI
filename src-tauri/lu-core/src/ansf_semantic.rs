//! M3-S1: rebuildable semantic projections over original ANSF SQLite entries.
//! The vectors are derived index bytes, never original messages or permissions.
//! Embedding computation is host-owned; this module does NOT invent embeddings,
//! access networks, load extension binaries or promote retrieved text to authority.
//!
//! Exact cosine scan is intentionally bounded: an optional, reviewed nearest-
//! neighbor extension can accelerate search later without changing source IDs.
use crate::ansf::{AnsfError,AnsfStore,Entry};
use crate::ansf_source_ids::AnsfSourceId;
use rusqlite::{params,Connection};
use sha2::{Digest,Sha256};
use std::collections::HashMap;

const MAX_DIMS:usize=2048;
const MAX_CHUNKS:i64=128;
const MAX_SCAN:usize=8192;
const MAX_HITS:usize=32;
const MAX_MODEL_ID:usize=128;

#[derive(Debug,Clone,PartialEq)]
pub struct SemanticHit{
    pub entry:Entry,
    pub chunk_ordinal:i64,
    pub similarity:f32,
}
#[derive(Debug,Clone,PartialEq)]
pub struct RankedSource{
    pub source_id:String,
    pub score:f32,
}
pub(crate) fn initialize_semantic_schema(conn:&Connection)->Result<(),AnsfError>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ansf_semantic_projection(
          entry_id INTEGER NOT NULL REFERENCES ansf_entries(id),
          chunk_ordinal INTEGER NOT NULL CHECK(chunk_ordinal>=0 AND chunk_ordinal<128),
          embedding_model TEXT NOT NULL,
          dimension INTEGER NOT NULL CHECK(dimension>=2 AND dimension<=2048),
          source_sha256 TEXT NOT NULL,
          vector_le_f32 BLOB NOT NULL,
          indexed_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          PRIMARY KEY(entry_id,chunk_ordinal,embedding_model)
        );
        CREATE INDEX IF NOT EXISTS ansf_semantic_by_model
          ON ansf_semantic_projection(embedding_model,entry_id);"
    )?;
    Ok(())
}
fn valid_model(model:&str)->bool{
    model.len()>=3 && model.len()<=MAX_MODEL_ID &&
    model.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'|b':'|b'@'))
}
fn vector_bytes(values:&[f32])->Result<Vec<u8>,AnsfError>{
    if !(2..=MAX_DIMS).contains(&values.len()) ||
       values.iter().any(|v|!v.is_finite()) ||
       values.iter().map(|v|(*v as f64)*(*v as f64)).sum::<f64>()==0.0 {
        return Err(AnsfError::InvalidInput)
    }
    let mut bytes=Vec::with_capacity(values.len()*4);
    for value in values{bytes.extend(value.to_le_bytes());}
    Ok(bytes)
}
fn digest(content:&str)->String{format!("{:x}",Sha256::digest(content.as_bytes()))}
fn similarity(query:&[f32],bytes:&[u8])->Result<f32,AnsfError>{
    if bytes.len()!=query.len()*4{return Err(AnsfError::InvalidInput)}
    let mut dot=0f64;
    let mut na=0f64;
    let mut nb=0f64;
    for (q,raw) in query.iter().zip(bytes.chunks_exact(4)){
        let other=f32::from_le_bytes([raw[0],raw[1],raw[2],raw[3]]);
        if !other.is_finite(){return Err(AnsfError::InvalidInput)}
        let a=*q as f64;let b=other as f64;
        dot+=a*b;na+=a*a;nb+=b*b;
    }
    if na==0.0||nb==0.0{return Err(AnsfError::InvalidInput)}
    Ok((dot/(na.sqrt()*nb.sqrt())).clamp(-1.0,1.0) as f32)
}
impl AnsfStore{
    /// Incremental local-only semantic indexing. The stored original text is
    /// returned only to the trusted host and a supervised local embedding
    /// model; the FTS/semantic projection never becomes a second transcript.
    /// Already indexed originals are skipped after crash/restart. Passage
    /// chunking beyond the first bounded text block is a later quality gate.
    pub fn semantic_pending_entries(
        &self,session_id:&str,embedding_model:&str,limit:usize
    )->Result<Vec<Entry>,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||
           session_id.contains('\0')||!valid_model(embedding_model)||
           limit==0||limit>24{return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT e.id,e.session_id,e.chapter_id,e.message_id,e.role,e.summary,e.content
             FROM ansf_entries e
             WHERE e.session_id=?1 AND e.role IN ('user','assistant')
               AND length(trim(e.content))>0
               AND NOT EXISTS(
                SELECT 1 FROM ansf_semantic_projection s
                WHERE s.entry_id=e.id AND s.embedding_model=?2 AND s.chunk_ordinal=0
               )
             ORDER BY e.id ASC LIMIT ?3"
        )?;
        stmt.query_map(params![session_id,embedding_model,limit as i64],|r|{
            Ok(Entry{
                id:r.get(0)?,session_id:r.get(1)?,chapter_id:r.get(2)?,
                message_id:r.get(3)?,role:r.get(4)?,summary:r.get(5)?,
                content:r.get(6)?
            })
        })?.collect::<Result<Vec<_>,_>>().map_err(Into::into)
    }
    pub fn semantic_indexed_count(
        &self,session_id:&str,embedding_model:&str
    )->Result<i64,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||
           session_id.contains('\0')||!valid_model(embedding_model){
            return Err(AnsfError::InvalidInput)
        }
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM ansf_semantic_projection s
             JOIN ansf_entries e ON e.id=s.entry_id
             WHERE e.session_id=?1 AND s.embedding_model=?2 AND s.chunk_ordinal=0",
            params![session_id,embedding_model],|r|r.get(0)
        )?)
    }

    /// Host-supplied deterministic embedding of a passage from an immutable
    /// original. Re-indexing identical bytes is idempotent; conflicting bytes
    /// under the same embedding model/version are refused, not overwritten.
    /// Model upgrades require a NEW model ID; old indexes remain rebuildable.
    pub fn index_semantic_passage(
        &self,session_id:&str,entry_id:i64,chunk_ordinal:i64,
        embedding_model:&str,embedding:&[f32]
    )->Result<(),AnsfError>{
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0')||
           entry_id<=0||!(0..MAX_CHUNKS).contains(&chunk_ordinal)||
           !valid_model(embedding_model){return Err(AnsfError::InvalidInput)}
        let bytes=vector_bytes(embedding)?;
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let (role,content):(String,String)=conn.query_row(
            "SELECT role,content FROM ansf_entries WHERE id=?1 AND session_id=?2",
            params![entry_id,session_id],
            |r|Ok((r.get(0)?,r.get(1)?))
        ).map_err(|_|AnsfError::InvalidInput)?;
        if !matches!(role.as_str(),"user"|"assistant"){return Err(AnsfError::InvalidInput)}
        let source_hash=digest(&content);
        conn.execute(
            "INSERT OR IGNORE INTO ansf_semantic_projection
             (entry_id,chunk_ordinal,embedding_model,dimension,source_sha256,vector_le_f32)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![entry_id,chunk_ordinal,embedding_model,embedding.len() as i64,
                source_hash,&bytes]
        )?;
        let existing:(i64,String,Vec<u8>)=conn.query_row(
            "SELECT dimension,source_sha256,vector_le_f32 FROM ansf_semantic_projection
             WHERE entry_id=?1 AND chunk_ordinal=?2 AND embedding_model=?3",
            params![entry_id,chunk_ordinal,embedding_model],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        )?;
        if existing.0!=embedding.len() as i64 || existing.1!=source_hash ||
            existing.2!=bytes{return Err(AnsfError::InvalidInput)}
        Ok(())
    }

    /// Exact bounded semantic search in ONE already-authorized ANSF book.
    /// Never use arbitrary model-selected paths. The caller must verify book
    /// identity and workspace before calling; this returns source entries only.
    /// A larger book must opt into a reviewed ANN backend rather than
    /// silently scan a partial prefix and claim complete recall.
    pub fn semantic_candidates(
        &self,session_id:&str,embedding_model:&str,query:&[f32],
        limit:usize
    )->Result<Vec<SemanticHit>,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0')||
           !valid_model(embedding_model)||limit==0||limit>MAX_HITS {
            return Err(AnsfError::InvalidInput)
        }
        vector_bytes(query)?;
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        let mut stmt=conn.prepare(
            "SELECT e.id,s.chunk_ordinal,s.dimension,s.source_sha256,s.vector_le_f32
             FROM ansf_semantic_projection s JOIN ansf_entries e ON s.entry_id=e.id
             WHERE e.session_id=?1 AND s.embedding_model=?2
             ORDER BY e.id,s.chunk_ordinal LIMIT ?3"
        )?;
        let rows=stmt.query_map(params![session_id,embedding_model,(MAX_SCAN+1) as i64],|r|{
            Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,
                r.get::<_,String>(3)?,r.get::<_,Vec<u8>>(4)?))
        })?;
        let mut best:HashMap<i64,(i64,f32,String)>=HashMap::new();
        let mut scanned=0usize;
        for row in rows{
            let (entry_id,chunk,dim,sha,vector)=row?;
            scanned+=1;
            if scanned>MAX_SCAN{return Err(AnsfError::InvalidInput)}
            if dim!=query.len() as i64{
                // A new model revision must use its own index ID.
                return Err(AnsfError::InvalidInput)
            }
            let score=similarity(query,&vector)?;
            best.entry(entry_id).and_modify(|prior|{
                if score>prior.1{*prior=(chunk,score,sha.clone())}
            }).or_insert((chunk,score,sha));
        }
        // No source message content is materialized during the vector scan.
        // Resolve only the bounded final candidates from the original book.
        let mut scores:Vec<_>=best.into_iter().collect();
        scores.sort_by(|a,b|b.1.1.total_cmp(&a.1.1).then(a.0.cmp(&b.0)));
        scores.truncate(limit);
        drop(stmt);drop(conn);
        let mut hits=Vec::with_capacity(scores.len());
        for (entry_id,(chunk,score,sha)) in scores{
            let entry=self.read_entry(session_id,entry_id)?
                .ok_or(AnsfError::InvalidInput)?;
            if digest(&entry.content)!=sha ||
                !matches!(entry.role.as_str(),"user"|"assistant"){
                return Err(AnsfError::InvalidInput)
            }
            hits.push(SemanticHit{entry,chunk_ordinal:chunk,similarity:score});
        }
        Ok(hits)
    }

    /// Remove only rebuildable, model-specific vector index rows. The ANSF
    /// original source entries, human chapters and source IDs remain intact.
    pub fn clear_semantic_model_index(
        &self,session_id:&str,embedding_model:&str
    )->Result<usize,AnsfError>{
        if session_id.is_empty()||session_id.len()>128||session_id.contains('\0')||
           !valid_model(embedding_model){return Err(AnsfError::InvalidInput)}
        let conn=self.conn.lock().map_err(|_|AnsfError::Locked)?;
        Ok(conn.execute(
            "DELETE FROM ansf_semantic_projection
             WHERE embedding_model=?1 AND entry_id IN
              (SELECT id FROM ansf_entries WHERE session_id=?2)",
            params![embedding_model,session_id]
        )?)
    }
}

/// M3-S2: reciprocal-rank fusion of two *ranked source ID lists*, not their
/// contents. The host must resolve selected IDs through the existing book
/// access policy. RRF weights ranks instead of comparing incomparable scores.
pub fn fuse_source_ranks(
    lexical:&[String],semantic:&[String],limit:usize
)->Result<Vec<RankedSource>,AnsfError>{
    if limit==0||limit>MAX_HITS||lexical.len()>128||semantic.len()>128{
        return Err(AnsfError::InvalidInput)
    }
    let mut result:HashMap<String,f32>=HashMap::new();
    for channel in [lexical,semantic]{
        let mut seen=std::collections::HashSet::new();
        for (i,id) in channel.iter().enumerate(){
            if AnsfSourceId::parse(id).is_err()||!seen.insert(id){continue}
            *result.entry(id.clone()).or_insert(0.0)+=1.0/(60.0+(i+1) as f32);
        }
    }
    let mut result:Vec<_>=result.into_iter().map(|(source_id,score)|
        RankedSource{source_id,score}).collect();
    result.sort_by(|a,b|b.score.total_cmp(&a.score)
        .then(a.source_id.cmp(&b.source_id)));
    result.truncate(limit);Ok(result)
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::ansf::{AnsfStore,EntryRole};
    #[test]
    fn semantic_index_ranks_synthetic_vectors_without_lexical_search(){
        let db=AnsfStore::in_memory().unwrap();
        db.create_session("c-1","local","Test").unwrap();
        db.start_chapter("c-1","Phase").unwrap();
        let a=db.append_entry("c-1","a",EntryRole::User,"cancel",
            "Worker cancellation survives a process crash").unwrap();
        let b=db.append_entry("c-1","b",EntryRole::Assistant,"other",
            "The nebula is bright").unwrap();
        // Exact synthetic vectors test the index/rank machinery, NOT an
        // embedding model's actual semantic comprehension.
        db.index_semantic_passage("c-1",a,0,"fixture-v1",&[1.0,0.0,0.0]).unwrap();
        db.index_semantic_passage("c-1",b,0,"fixture-v1",&[0.0,1.0,0.0]).unwrap();
        let hits=db.semantic_candidates("c-1","fixture-v1",&[0.9,0.1,0.0],2).unwrap();
        assert_eq!(hits[0].entry.id,a);
        assert_eq!(hits[1].entry.id,b);
        assert!(hits[0].similarity>0.9);
    }
    #[test]
    fn rebuild_and_model_versions_do_not_change_original_entries(){
        let db=AnsfStore::in_memory().unwrap();
        db.create_session("c-2","local","Test").unwrap();
        db.start_chapter("c-2","Phase").unwrap();
        let id=db.append_entry("c-2","a",EntryRole::User,"Decision","Final answer").unwrap();
        db.index_semantic_passage("c-2",id,0,"model-v1",&[1.0,0.0]).unwrap();
        db.index_semantic_passage("c-2",id,0,"model-v1",&[1.0,0.0]).unwrap();
        assert!(db.index_semantic_passage("c-2",id,0,"model-v1",&[0.0,1.0]).is_err());
        db.index_semantic_passage("c-2",id,0,"model-v2",&[0.0,1.0]).unwrap();
        assert_eq!(db.clear_semantic_model_index("c-2","model-v1").unwrap(),1);
        assert!(db.semantic_candidates("c-2","model-v1",&[1.0,0.0],5).unwrap().is_empty());
        assert_eq!(db.semantic_candidates("c-2","model-v2",&[0.0,1.0],5).unwrap().len(),1);
        assert_eq!(db.read_entry("c-2",id).unwrap().unwrap().content,"Final answer");
    }
    #[test]
    fn malformed_zero_nan_non_user_and_dimension_conflicts_fail_closed(){
        let db=AnsfStore::in_memory().unwrap();
        db.create_session("c-3","local","Test").unwrap();
        db.start_chapter("c-3","Phase").unwrap();
        let call=db.append_entry("c-3","tool",EntryRole::ToolResult,"Observed","No authority").unwrap();
        assert!(db.index_semantic_passage("c-3",call,0,"model-v1",&[1.0,0.0]).is_err());
        let id=db.append_entry("c-3","a",EntryRole::Assistant,"Answer","Hello").unwrap();
        for bad in [&[0.0f32,0.0][..],&[f32::NAN,1.0][..]]{
            assert!(db.index_semantic_passage("c-3",id,0,"model-v1",bad).is_err());
        }
        db.index_semantic_passage("c-3",id,0,"model-v1",&[0.0,1.0]).unwrap();
        assert!(db.semantic_candidates("c-3","model-v1",&[1.0,0.0,0.0],5).is_err());
        assert!(db.semantic_candidates("c-3","model-v1",&[0.0,0.0],5).is_err());
        assert!(db.semantic_candidates("c-3","bad;id",&[0.0,1.0],5).is_err());
    }
    #[test]
    fn incremental_original_index_skips_completed_entries_after_restart(){
        let db=AnsfStore::in_memory().unwrap();
        db.create_session("c-index","local","Test").unwrap();
        db.start_chapter("c-index","Decision").unwrap();
        let a=db.append_entry("c-index","a",EntryRole::User,"Task","Where is the installer?").unwrap();
        let b=db.append_entry("c-index","b",EntryRole::Assistant,"Answer","Stored in local build assets").unwrap();
        let first=db.semantic_pending_entries("c-index","model-v1",1).unwrap();
        assert_eq!(first[0].id,a);
        db.index_semantic_passage("c-index",a,0,"model-v1",&[0.9,0.1]).unwrap();
        assert_eq!(db.semantic_indexed_count("c-index","model-v1").unwrap(),1);
        let second=db.semantic_pending_entries("c-index","model-v1",12).unwrap();
        assert_eq!(second.len(),1);
        assert_eq!(second[0].id,b);
        assert_eq!(db.semantic_pending_entries("c-index","model-v2",12).unwrap().len(),2);
        assert!(db.semantic_pending_entries("c-index","model-v1",25).is_err());
    }
    #[test]
    fn source_fusion_prefers_matches_retrieved_by_both_channels(){
        let s=|i|AnsfSourceId::new("c-1",i).unwrap().encode();
        let ranks=fuse_source_ranks(&[s(1),s(2)],&[s(2),s(3)],3).unwrap();
        assert_eq!(ranks[0].source_id,s(2));
        assert_eq!(ranks.len(),3);
    }
}
