//! X's pay-per-use prices, from
//! <https://docs.x.com/x-api/getting-started/pricing>. Each rate quotes the
//! page's row; reads are per resource returned, writes per request.

// "Posts: Read | $0.005 per resource"
pub(super) const POST_READ_USD: f64 = 0.005;
// "User: Read | $0.010 per resource"
pub(super) const USER_READ_USD: f64 = 0.010;
// Owned Reads: "These endpoints are priced at $0.001 per resource"
pub(super) const OWNED_READ_USD: f64 = 0.001;
// "Post: Create | $0.015 per request"
const POST_CREATE_USD: f64 = 0.015;
// "Post: Create (with URL) | $0.200 per request"
const POST_CREATE_WITH_URL_USD: f64 = 0.200;
// "Post: Create (summoned) | $0.010 per request"
const POST_CREATE_SUMMONED_USD: f64 = 0.010;
// "DM Interaction: Create | $0.015 per request"
const DM_INTERACTION_CREATE_USD: f64 = 0.015;
// "User Interaction: Create | $0.015 per request"
const USER_INTERACTION_CREATE_USD: f64 = 0.015;
// "Interaction: Delete | $0.010 per request"
const INTERACTION_DELETE_USD: f64 = 0.010;
// "Mute: Delete | $0.005 per request"
const MUTE_DELETE_USD: f64 = 0.005;

/// `GET /2/users/{id}/<suffix>` endpoints the pricing page lists as Owned Reads.
pub(super) const OWNED_READ_SUFFIXES: &[&str] = &[
    "tweets",
    "mentions",
    "liked_tweets",
    "bookmarks",
    "followers",
    "following",
    "blocking",
    "muting",
    "owned_lists",
    "followed_lists",
    "list_memberships",
    "pinned_lists",
];

/// The write rows of X's price table that bird's requests fall under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteRate {
    PostCreate,
    PostCreateWithUrl,
    PostCreateSummoned,
    DmInteractionCreate,
    UserInteractionCreate,
    InteractionDelete,
    MuteDelete,
}

impl WriteRate {
    pub fn usd(self) -> f64 {
        match self {
            Self::PostCreate => POST_CREATE_USD,
            Self::PostCreateWithUrl => POST_CREATE_WITH_URL_USD,
            Self::PostCreateSummoned => POST_CREATE_SUMMONED_USD,
            Self::DmInteractionCreate => DM_INTERACTION_CREATE_USD,
            Self::UserInteractionCreate => USER_INTERACTION_CREATE_USD,
            Self::InteractionDelete => INTERACTION_DELETE_USD,
            Self::MuteDelete => MUTE_DELETE_USD,
        }
    }

    /// The row's name on X's pricing page.
    pub fn label(self) -> &'static str {
        match self {
            Self::PostCreate => "Post: Create",
            Self::PostCreateWithUrl => "Post: Create (with URL)",
            Self::PostCreateSummoned => "Post: Create (summoned)",
            Self::DmInteractionCreate => "DM Interaction: Create",
            Self::UserInteractionCreate => "User Interaction: Create",
            Self::InteractionDelete => "Interaction: Delete",
            Self::MuteDelete => "Mute: Delete",
        }
    }
}
