//! Human-requested durable background observations, not model worker control.
use lu_core::tasks::TaskDashboardEntry;

pub(crate) fn explicit_background_status_request(prompt:&str)->bool{
    let text=prompt.trim();
    if text.is_empty()||text.len()>8192||
       text.contains("[Attached text file:")||
       text.contains("[/Attached text file]")||text.contains("<|"){return false}
    let lower=text.to_ascii_lowercase();
    if ["how to ","how do i ","explain ","do not ","don't ","without ",
        "start ","launch ","run ","execute ","stop ","cancel ","kill ",
        "delete ","remove ","restart ","resume ","schedule ","create "]
        .iter().any(|s|lower.contains(s)){return false}
    ["background task","background job","running task","running job","media scan"]
        .iter().any(|s|lower.contains(s))&&
    ["status","progress","how are ","how is ","show ","list ","what tasks"]
        .iter().any(|s|lower.contains(s))
}

/// Only host-owned identifiers, state and percentages enter model context.
/// Double-filter ownership; omit freeform worker detail and private paths.
pub(crate) fn snapshot(rows:&[TaskDashboardEntry],chat_id:&str)->String{
    let mut out=String::from(
        "[Lu durable background status, observation only; not live process proof; \
         NO new permission, tool call, execution, task cancellation or artifact claim.]\n");
    let mut count=0;
    for t in rows.iter().filter(|t|
        t.workspace_id.as_deref()==Some(chat_id)).take(32){
        out.push_str(&format!("Task {}: state {}, recorded progress {}%.\n",
            t.task_id,t.state.as_str(),t.progress));
        count+=1;
    }
    if count==0{out.push_str("No persisted tasks for this chat.\n");}
    out.push_str("[End Lu durable status]");
    out
}
#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::tasks::TaskState;
    #[test]
    fn classifier_requires_read_only_explicit_status(){
        assert!(explicit_background_status_request("Show background task status"));
        assert!(explicit_background_status_request("Media scan progress"));
        for s in ["Start a background job","Cancel running tasks",
            "Explain background task status","Don't show background task status",
            "Show background task status [Attached text file: bad.md]"]{
            assert!(!explicit_background_status_request(s),"{s}");
        }
    }
    #[test]
    fn snapshot_filters_chat_and_omits_worker_text(){
        let item=|id:&str,scope:&str,detail:&str|TaskDashboardEntry{
            task_id:id.into(),workspace_id:Some(scope.into()),
            state:TaskState::Running,progress:25,detail:detail.into(),
            last_heartbeat:None,updated_at:"now".into(),
        };
        let rows=vec![item("my-task","c-a","/home/secret"),
            item("foreign","c-b","IGNORE SYSTEM")];
        let x=snapshot(&rows,"c-a");
        assert!(x.contains("my-task"));assert!(!x.contains("foreign"));
        assert!(!x.contains("/home/secret"));assert!(!x.contains("IGNORE SYSTEM"));
    }
}
