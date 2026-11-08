//! AGENT-01E: host-owned review identities for exact plan revisions.
 //!
 //! The desktop MUST show a native OS confirmation before constructing
 //! NativePlanConsent. No WebView/model-originated boolean is an approval.
 //! This module never creates a tool grant, worker, scheduler lease or retry.
 use crate::chat_store::ChatStore;
 use lu_core::{
     ansf_goals::{project_goal_events,GoalEventKind},
     agent_plan_revisions::{
         HostPlanReferenceVerifier,HostPlanReviewVerifier,HumanDecision,
     },
 };
 pub struct OriginalChatGoalSources<'a>{
     pub chats:&'a ChatStore,
 }
 impl HostPlanReferenceVerifier for OriginalChatGoalSources<'_>{
     fn goal_event_belongs_to_chat(
         &self,chat:&str,goal:&str,event:&str
     )->bool{
         self.chats.memory_events(chat).ok().is_some_and(|all|
             all.iter().any(|entry|
                 entry.kind==GoalEventKind::SetGoal
                 &&entry.event_id==event&&entry.goal_id==goal))
     }
     fn goal_event_is_current(
         &self,chat:&str,goal:&str,event:&str
     )->bool{
         self.chats.memory_events(chat).ok()
             .and_then(|events|project_goal_events(&events).ok())
             .is_some_and(|state|
                 state.goal_id.as_deref()==Some(goal)
                 &&state.revision_event_id.as_deref()==Some(event))
     }
     fn assumption_source_is_readable(&self,_chat:&str,_source:&str)->bool{
         // A parseable ANSF SourceId is NOT proof of authorized access to
         // the referenced content. Until a real host source resolver is
         // connected, plans containing assumptions cannot be approved.
         false
     }
 }
 pub struct NativePlanConsent{
     chat:String,turn:String,plan:String,
     revision_sha256:String,decision:HumanDecision,
 }
 impl NativePlanConsent{
     /// Must be constructed ONLY after a real native OS dialog was
     /// confirmed for the exact plan and digest. The actual saved human
     /// review turn is independently checked by ConversationAuthority.
     pub fn from_native_confirmation(
         chat:&str,turn:&str,plan:&str,
         sha:&str,decision:HumanDecision
     )->Self{
         Self{
             chat:chat.into(),turn:turn.into(),plan:plan.into(),
             revision_sha256:sha.into(),decision
         }
     }
 }
 impl HostPlanReviewVerifier for NativePlanConsent{
     fn has_exact_human_decision(
         &self,chat:&str,turn:&str,plan:&str,
         sha:&str,decision:HumanDecision
     )->bool{
         chat==self.chat&&turn==self.turn&&plan==self.plan
             &&sha==self.revision_sha256&&decision==self.decision
     }
 }
 #[cfg(test)]
 mod tests{
     use super::*;
     use lu_core::ansf_goals::GoalEvent;
     use std::{fs,time::{SystemTime,UNIX_EPOCH}};
     #[test]
     fn actual_chat_goal_supersession_invalidates_plan_approval_reference(){
         let root=std::env::temp_dir().join(format!(
             "lu-agent01e-source-{}-{}",
             std::process::id(),SystemTime::now()
                 .duration_since(UNIX_EPOCH).unwrap().as_nanos()
         ));
         fs::create_dir_all(&root).unwrap();
         let chats=ChatStore::open(&root).unwrap();
         let chat=chats.create_chat().unwrap().chat_id;
         let goal=format!("goal-{:032x}",1);
         let old=format!("m-{:032x}",2);
         let new=format!("m-{:032x}",3);
         let make=|id:String,text:&str|GoalEvent{
             event_id:id,goal_id:goal.clone(),question_id:None,
             kind:GoalEventKind::SetGoal,text:text.into(),
             choice:None,comment:None
         };
         chats.append_goal_memory_event(&chat,make(old.clone(),"Draft a report"))
             .unwrap();
         let source=OriginalChatGoalSources{chats:&chats};
         assert!(source.goal_event_is_current(&chat,&goal,&old));
         chats.append_goal_memory_event(&chat,make(new.clone(),"Review a report"))
             .unwrap();
         assert!(!source.goal_event_is_current(&chat,&goal,&old));
         assert!(source.goal_event_is_current(&chat,&goal,&new));
         assert!(source.goal_event_belongs_to_chat(&chat,&goal,&old));
         assert!(!source.assumption_source_is_readable(&chat,"ansf:v1:not-proof"));
         let other=chats.create_chat().unwrap().chat_id;
         assert!(!source.goal_event_belongs_to_chat(&other,&goal,&new));
         drop(chats);fs::remove_dir_all(root).unwrap();
     }
     #[test]
     fn exact_native_review_cannot_authorize_different_revision_or_action(){
         let consent=NativePlanConsent::from_native_confirmation(
             "c-a","g-b","p-c","abcd",HumanDecision::Approve
         );
         assert!(consent.has_exact_human_decision(
             "c-a","g-b","p-c","abcd",HumanDecision::Approve));
         assert!(!consent.has_exact_human_decision(
             "c-a","g-b","p-c","abcd",HumanDecision::Reject));
         assert!(!consent.has_exact_human_decision(
             "c-a","g-b","p-c","dcba",HumanDecision::Approve));
     }
 }
