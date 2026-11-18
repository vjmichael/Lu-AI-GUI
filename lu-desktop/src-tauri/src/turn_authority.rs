//! CP-05A: shared persisted-human verification for scoped native turns.
//! This is provenance only. It never opens a grant, chooses a resource or
//! makes an authorization durable across restart. Scope, consent, generation
//! ownership and cancellation remain mandatory in each admission handler.
use crate::chat_store::ChatStore;
use lu_core::conversation_authority::TrustedUserTurnVerifier;

#[derive(Clone,Copy)]
enum RequiredHumanEvent { QueuedChat, CurrentChat, QueuedReader }

pub(crate) struct PersistedHumanTurn<'a>{
    chats:&'a ChatStore, chat:&'a str, turn:&'a str, expected:&'a str,
    event:RequiredHumanEvent,
}
impl<'a> PersistedHumanTurn<'a>{
    pub(crate) fn queued_chat(chats:&'a ChatStore,chat:&'a str,turn:&'a str,expected:&'a str)->Self{
        Self{chats,chat,turn,expected,event:RequiredHumanEvent::QueuedChat}
    }
    pub(crate) fn current_chat(chats:&'a ChatStore,chat:&'a str,turn:&'a str,expected:&'a str)->Self{
        Self{chats,chat,turn,expected,event:RequiredHumanEvent::CurrentChat}
    }
    pub(crate) fn queued_reader(chats:&'a ChatStore,chat:&'a str,turn:&'a str,expected:&'a str)->Self{
        Self{chats,chat,turn,expected,event:RequiredHumanEvent::QueuedReader}
    }
}
impl TrustedUserTurnVerifier for PersistedHumanTurn<'_>{
    fn verify_user_turn(&self,chat:&str,turn:&str)->bool{
        if chat!=self.chat||turn!=self.turn{return false}
        match self.event{
            RequiredHumanEvent::QueuedReader=>self.chats.reader_user_turn_matches(chat,turn,self.expected).unwrap_or(false),
            RequiredHumanEvent::QueuedChat|RequiredHumanEvent::CurrentChat=>{
                self.chats.trusted_user_turn_text(chat,turn).as_deref()==Some(self.expected)&&
                (!matches!(self.event,RequiredHumanEvent::CurrentChat)||
                    self.chats.latest_authenticated_user_turn_id(chat).as_deref()==Some(turn))
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn persisted_event_kind_exact_text_and_queue_freshness_remain_distinct(){
        let root=std::env::temp_dir().join(crate::next_id("lu-cp05-authority-"));
        let chats=ChatStore::open(&root).unwrap();
        let a=chats.create_chat().unwrap().chat_id;
        let b=chats.create_chat().unwrap().chat_id;
        let chat=crate::next_id("g-");
        let reader=crate::next_id("g-");
        let latest=crate::next_id("g-");
        let text="Read this document";
        let queued=PersistedHumanTurn::queued_chat(&chats,&a,&chat,text);
        let current=PersistedHumanTurn::current_chat(&chats,&a,&chat,text);
        let document=PersistedHumanTurn::queued_reader(&chats,&a,&reader,text);
        assert!(!queued.verify_user_turn(&a,&chat));
        chats.append_user(&a,text).unwrap();
        chats.append_user_with_turn_id(&b,&chat,text).unwrap();
        chats.append_reader_user(&a,&reader,text).unwrap();
        assert!(!queued.verify_user_turn(&a,&chat));
        assert!(!current.verify_user_turn(&b,&chat));
        assert!(document.verify_user_turn(&a,&reader));
        assert!(!PersistedHumanTurn::queued_chat(&chats,&a,&reader,text).verify_user_turn(&a,&reader));
        chats.append_user_with_turn_id(&a,&chat,text).unwrap();
        assert!(queued.verify_user_turn(&a,&chat));
        assert!(current.verify_user_turn(&a,&chat));
        assert!(!PersistedHumanTurn::queued_reader(&chats,&a,&chat,text).verify_user_turn(&a,&chat));
        assert!(!PersistedHumanTurn::queued_chat(&chats,&a,&chat,"Changed text").verify_user_turn(&a,&chat));
        assert!(!queued.verify_user_turn(&a,&reader));
        chats.append_user_with_turn_id(&a,&latest,"New question").unwrap();
        assert!(queued.verify_user_turn(&a,&chat));
        assert!(!current.verify_user_turn(&a,&chat));
        assert!(document.verify_user_turn(&a,&reader));
        drop(chats);
        let reopened=ChatStore::open(&root).unwrap();
        assert!(PersistedHumanTurn::queued_chat(&reopened,&a,&chat,text).verify_user_turn(&a,&chat));
        assert!(!PersistedHumanTurn::current_chat(&reopened,&a,&chat,text).verify_user_turn(&a,&chat));
        drop(reopened);
        let _=std::fs::remove_dir_all(root);
    }
}
