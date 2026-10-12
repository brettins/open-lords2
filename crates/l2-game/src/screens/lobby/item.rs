//! The lobby's pages and buttons; the words are ours (rule 6, C255).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Entry,
    Join,
    Room,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Host,
    Join,
    Resume,
    Connect,
    UseAddress,
    UseCode,
    Back,
    Start,
    Ready,
    NotReady,
    Leave,
}

impl Item {
    pub fn label(self) -> &'static str {
        match self {
            Item::Host => "Host a game",
            Item::Join => "Join a game",
            Item::Resume => "Continue a saved game",
            Item::Connect => "Join",
            Item::UseAddress => "use an address instead",
            Item::UseCode => "use a code instead",
            Item::Back => "Back",
            Item::Start => "Start game",
            Item::Ready => "I'm ready",
            Item::NotReady => "Not ready",
            Item::Leave => "Leave",
        }
    }
}
