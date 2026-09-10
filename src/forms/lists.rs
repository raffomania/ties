use garde::Validate;
use serde::Deserialize;

#[derive(Validate, Default, Deserialize)]
pub struct CreateList {
    #[garde(length(min = 1, max = 100))]
    pub title: String,
    #[garde(skip)]
    pub content: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub private: bool,
}

#[derive(Validate, Deserialize, Default)]
pub struct EditTitle {
    #[garde(length(min = 1, max = 100))]
    pub title: String,
}

#[derive(Deserialize)]
pub struct EditListPrivate {
    pub private: bool,
}

#[derive(Deserialize)]
pub struct EditListPinned {
    pub pinned: bool,
}
