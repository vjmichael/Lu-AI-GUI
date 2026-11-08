//! E1-P12: direct MP4/ISO-BMFF metadata oracle. No FFprobe or shell.
//! Deliberately strict: validates a single non-fragmented ftyp/moov/mdat
//! file with explicit movie/track timescales and ordinary AVC/HEVC/AAC
//! sample descriptions. Refuses edits, multiple stsd entries, protection,
//! references, fragments, structural ambiguity and unrecognized codecs.
//! Independently checks container duration, per-track codec, dimensions,
//! channels and sample rate against the supervised FFprobe receipt.
use serde_json::Value;

const MAX_TRACKS:usize=16;
const MAX_DEPTH:usize=10;
const MAX_BOXES:usize=4096;
const MAX_DURATION_MS:u64=7*24*60*60*1000;

#[derive(Clone,Copy,Debug)]
struct Atom<'a>{kind:[u8;4],value:&'a [u8]}
fn be16(v:&[u8])->Option<u16>{Some(u16::from_be_bytes(v.get(..2)?.try_into().ok()?))}
fn be32(v:&[u8])->Option<u32>{Some(u32::from_be_bytes(v.get(..4)?.try_into().ok()?))}
fn be64(v:&[u8])->Option<u64>{Some(u64::from_be_bytes(v.get(..8)?.try_into().ok()?))}
fn kind(atom:&Atom<'_>,name:&[u8;4])->bool{&atom.kind==name}
fn atoms<'a>(bytes:&'a [u8],depth:usize,budget:&mut usize)->Option<Vec<Atom<'a>>>{
    if depth>MAX_DEPTH{return None}
    let mut pos=0usize;
    let mut out=Vec::new();
    while pos<bytes.len(){
        if *budget==0||bytes.len()-pos<8{return None}
        *budget-=1;
        let base=pos;
        let size=be32(bytes.get(base..base+4)?)? as u64;
        let mut name=[0;4];name.copy_from_slice(bytes.get(base+4..base+8)?);
        let (total,header)=if size==1{
            if bytes.len()-base<16{return None}
            (be64(bytes.get(base+8..base+16)?)?,16usize)
        }else if size==0{
            // Ambiguous lengths are not allowed for independently asserted facts.
            return None
        }else{(size,8usize)};
        let total=usize::try_from(total).ok()?;
        if total<header||total>bytes.len()-base{return None}
        pos=base.checked_add(total)?;
        out.push(Atom{kind:name,value:bytes.get(base+header..pos)?});
    }
    Some(out)
}
fn unique<'a,'b>(items:&'b [Atom<'a>],name:&[u8;4])->Option<&'b Atom<'a>>{
    let mut found=None;
    for item in items.iter().filter(|x|kind(x,name)){
        if found.is_some(){return None}
        found=Some(item)
    }
    found
}
fn timescale_and_duration(data:&[u8])->Option<(u64,u64)>{
    let version=*data.first()?;
    if data.len()<24{return None}
    let (timescale,duration)=match version{
        0=>(be32(data.get(12..16)?)? as u64,
            be32(data.get(16..20)?)? as u64),
        1=>(be32(data.get(20..24)?)? as u64,
            be64(data.get(24..32)?)?),
        _=>return None
    };
    if timescale==0||duration==0||duration==u32::MAX as u64
        ||duration==u64::MAX{return None}
    Some((timescale,duration))
}
fn ms(duration:u64,scale:u64)->Option<u64>{
    let value=(duration as u128).checked_mul(1000)?
        .checked_add(scale as u128/2)?
        .checked_div(scale as u128)?;
    let out=u64::try_from(value).ok()?;
    (out>0&&out<=MAX_DURATION_MS).then_some(out)
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VideoAudioFacts{
    pub duration_millis:u64,
    pub tracks:Vec<TrackFacts>,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TrackFacts{
    pub codec:&'static str,
    pub kind:&'static str,
    pub width:Option<u64>,
    pub height:Option<u64>,
    pub channels:Option<u64>,
    pub sample_rate:Option<u64>,
}
fn parse_track(trak:&Atom<'_>,budget:&mut usize)->Option<TrackFacts>{
    let children=atoms(trak.value,1,budget)?;
    // An edit list changes the presentation timeline; the simple source
    // observer does not yet implement edit-list time remapping.
    if children.iter().any(|a|
        matches!(&a.kind,b"edts"|b"tref"|b"sinf")){return None}
    let tkhd=unique(&children,b"tkhd")?;
    if tkhd.value.is_empty()||tkhd.value[0]>1{return None}
    let mdia=*unique(&children,b"mdia")?;
    let mdia_children=atoms(mdia.value,2,budget)?;
    let hdlr=unique(&mdia_children,b"hdlr")?;
    let handler=hdlr.value.get(8..12)?;
    let kind=if handler==b"vide"{"video"}else if handler==b"soun"{"audio"}
        else{return None};
    let mdhd=unique(&mdia_children,b"mdhd")?;
    let (rate,duration)=timescale_and_duration(mdhd.value)?;
    ms(duration,rate)?;
    let minf=unique(&mdia_children,b"minf")?;
    let minf_children=atoms(minf.value,3,budget)?;
    let stbl=unique(&minf_children,b"stbl")?;
    let table=atoms(stbl.value,4,budget)?;
    let stsd=unique(&table,b"stsd")?;
    let format=stsd.value;
    if format.len()<16||format[0]!=0||format[1..4]!=[0,0,0]
        ||be32(format.get(4..8)?)?!=1{return None}
    let entries=atoms(&format[8..],5,budget)?;
    if entries.len()!=1{return None}
    let entry=entries[0];
    if entry.value.len()<28||entry.value[..6]!=[0;6]
        ||be16(entry.value.get(6..8)?)?==0{return None}
    if kind=="video"{
        if entry.value.len()<78{return None}
        let codec=match &entry.kind{
            b"avc1"|"avc3"=>"h264",
            b"hvc1"|"hev1"=>"hevc",
            b"av01"=>"av1",
            b"vp09"=>"vp9",
            b"mp4v"=>"mpeg4",
            _=>return None
        };
        // Codec configuration must be present, otherwise a forged
        // header could appear to identify actual video content.
        let nested=atoms(&entry.value[78..],6,budget)?;
        let config=match codec{
            "h264"=>b"avcC".as_slice(),
            "hevc"=>b"hvcC".as_slice(),
            "av1"=>b"av1C".as_slice(),
            "mpeg4"=>b"esds".as_slice(),
            _=>b"vpcC".as_slice()
        };
        if !nested.iter().any(|x|x.kind.as_slice()==config
            &&(if codec=="mpeg4"{
                x.value.len()>=12&&x.value[..4]==[0,0,0,0]
                    &&has_esds_object_type(&x.value[4..],0x20)
            }else{!x.value.is_empty()})){return None}
        let width=be16(entry.value.get(24..26)?)? as u64;
        let height=be16(entry.value.get(26..28)?)? as u64;
        if !(1..=16384).contains(&width)||!(1..=16384).contains(&height){
            return None
        }
        Some(TrackFacts{codec,kind,width:Some(width),height:Some(height),
            channels:None,sample_rate:None})
    }else{
        if entry.value.len()<28||&entry.kind!=b"mp4a"{return None}
        let channels=be16(entry.value.get(16..18)?)? as u64;
        let version=be16(entry.value.get(8..10)?)?;
        let fixed_rate=be32(entry.value.get(24..28)?)? as u64;
        let sample_rate=fixed_rate>>16;
        if version!=0||!(1..=64).contains(&channels)
            ||!(1..=768000).contains(&sample_rate)
            ||fixed_rate&0xffff!=0{return None}
        let nested=atoms(&entry.value[28..],6,budget)?;
        let esds=unique(&nested,b"esds")?;
        if esds.value.len()<12||esds.value[..4]!=[0,0,0,0]
            ||!has_esds_object_type(&esds.value[4..],0x40){return None}
        Some(TrackFacts{codec:"aac",kind,width:None,height:None,
            channels:Some(channels),sample_rate:Some(sample_rate)})
    }
}
/// Locate a correctly delimited MPEG-4 DecoderConfigDescriptor (tag 0x04)
/// within the ordinary ES_Descriptor sequence (tag 0x03), and require
/// MPEG-4 Audio ObjectTypeIndication 0x40. Never infer AAC from mp4a alone.
fn has_esds_object_type(bytes:&[u8],requested:u8)->bool{
    let Some((outer,next))=descriptor(bytes,0) else{return false};
    if outer.0!=3{return false}
    let payload=&bytes[outer.1..next];
    if payload.len()<5{return false}
    let flags=payload[2];
    let mut offset=3usize;
    if flags&0x80!=0{offset=offset.saturating_add(2)}
    if flags&0x40!=0{
        let Some(&length)=payload.get(offset) else{return false};
        offset=offset.saturating_add(1+length as usize)
    }
    if flags&0x20!=0{offset=offset.saturating_add(2)}
    if offset>=payload.len(){return false}
    let Some((desc,end))=descriptor(payload,offset) else{return false};
    desc.0==4&&end<=payload.len()&&payload.get(desc.1)==Some(&requested)
}
fn descriptor(bytes:&[u8],start:usize)->Option<((u8,usize),usize)>{
    let tag=*bytes.get(start)?;
    let mut idx=start+1;
    let mut size=0usize;
    let mut terminal=false;
    for _ in 0..4{
        let b=*bytes.get(idx)?;idx+=1;
        size=size.checked_mul(128)?.checked_add((b&0x7f) as usize)?;
        if b&0x80==0{terminal=true;break}
    }
    if !terminal||size>1024*1024{return None}
    let end=idx.checked_add(size)?;
    if end>bytes.len(){return None}
    Some(((tag,idx),end))
}
pub fn parse_mp4(bytes:&[u8])->Option<VideoAudioFacts>{
    if bytes.len()<128||bytes.len()>256*1024*1024{return None}
    let mut budget=MAX_BOXES;
    let boxes=atoms(bytes,0,&mut budget)?;
    if boxes.iter().filter(|a|kind(a,b"ftyp")).count()!=1
        ||boxes.iter().filter(|a|kind(a,b"mdat")).count()!=1
        ||boxes.iter().filter(|a|kind(a,b"moov")).count()!=1
        ||boxes.iter().any(|a|matches!(&a.kind,b"moof"|b"mfra"|b"mvex")){
        return None
    }
    let ftyp=unique(&boxes,b"ftyp")?;
    if ftyp.value.len()<8{return None}
    let moov=unique(&boxes,b"moov")?;
    let moov_children=atoms(moov.value,1,&mut budget)?;
    if moov_children.iter().any(|a|kind(a,b"mvex")){return None}
    let mvhd=unique(&moov_children,b"mvhd")?;
    let (scale,duration)=timescale_and_duration(mvhd.value)?;
    let duration_millis=ms(duration,scale)?;
    let tracks=moov_children.iter().filter(|a|kind(a,b"trak"))
        .collect::<Vec<_>>();
    if tracks.is_empty()||tracks.len()>MAX_TRACKS{return None}
    let mut parsed=Vec::with_capacity(tracks.len());
    for track in tracks{
        parsed.push(parse_track(track,&mut budget)?)
    }
    Some(VideoAudioFacts{duration_millis,tracks:parsed})
}
pub fn matches_ffprobe(facts:&VideoAudioFacts,receipt:&Value)->bool{
    let Some(actual)=receipt.get("durationMilliseconds").and_then(Value::as_u64)
        else{return false};
    if actual.abs_diff(facts.duration_millis)>5{return false}
    let Some(streams)=receipt.get("streams").and_then(Value::as_array)
        else{return false};
    if streams.len()!=facts.tracks.len(){return false}
    for (i,stream) in streams.iter().enumerate(){
        let source=&facts.tracks[i];
        if stream.get("index").and_then(Value::as_u64)!=Some(i as u64)
            ||stream.get("type").and_then(Value::as_str)!=Some(source.kind)
            ||stream.get("codec").and_then(Value::as_str)!=Some(source.codec){
            return false
        }
        let equal=|field:&str,expect:Option<u64>|{
            let value=stream.get(field).and_then(Value::as_u64);
            value==expect
        };
        if !equal("width",source.width)||!equal("height",source.height)
            ||!equal("channels",source.channels)
            ||!equal("sampleRateHz",source.sample_rate){
            return false
        }
    }
    true
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    fn atom(name:&[u8;4],v:&[u8])->Vec<u8>{
        let mut b=Vec::new();b.extend_from_slice(&((v.len()+8) as u32).to_be_bytes());
        b.extend_from_slice(name);b.extend_from_slice(v);b
    }
    #[test]
    fn malformed_top_level_and_fragmented_boxes_are_rejected(){
        assert!(parse_mp4(b"not a movie").is_none());
        let forged=atom(b"ftyp",b"isom0000");
        assert!(parse_mp4(&forged).is_none());
        let mut invalid=forged.clone();
        invalid.extend(atom(b"moof",b"fragment"));
        invalid.extend(atom(b"mdat",b"data"));
        invalid.extend(atom(b"moov",b"fake"));
        assert!(parse_mp4(&invalid).is_none());
    }
    #[test]
    fn independent_duration_and_matching_streams_are_not_model_claims(){
        let facts=VideoAudioFacts{duration_millis:1000,tracks:vec![
            TrackFacts{kind:"video",codec:"h264",width:Some(640),
                height:Some(360),channels:None,sample_rate:None},
            TrackFacts{kind:"audio",codec:"aac",width:None,height:None,
                channels:Some(2),sample_rate:Some(48000)}
        ]};
        let good=json!({"durationMilliseconds":1000,"streams":[
            {"index":0,"type":"video","codec":"h264","width":640,
             "height":360,"channels":null,"sampleRateHz":null},
            {"index":1,"type":"audio","codec":"aac","width":null,
             "height":null,"channels":2,"sampleRateHz":48000}
        ]});
        assert!(matches_ffprobe(&facts,&good));
        let mut forged=good.clone();forged["streams"][1]["channels"]=json!(1);
        assert!(!matches_ffprobe(&facts,&forged));
        forged=good.clone();forged["durationMilliseconds"]=json!(2000);
        assert!(!matches_ffprobe(&facts,&forged));
    }
}
