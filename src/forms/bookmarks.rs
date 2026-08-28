use garde::Validate;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{db::bookmarks::InsertBookmark, form_errors::FormErrors};

/// Form for inserting a bookmark with an empty title.
#[derive(Validate, Default, Deserialize, Clone, Debug, Serialize)]
pub struct CreateBookmark {
    #[garde(url)]
    pub url: String,
}

impl TryFrom<CreateBookmark> for InsertBookmark {
    type Error = FormErrors;

    fn try_from(value: CreateBookmark) -> Result<Self, Self::Error> {
        value.validate()?;

        Ok(InsertBookmark {
            url: value.url,
            title: None,
        })
    }
}

#[derive(Validate, Default, Deserialize, Clone, Debug)]
pub struct Rename {
    #[garde(length(min = 1, max = 500))]
    pub title: String,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Disconnect {
    pub delete_link_id: Uuid,
}

#[derive(Validate, Default, Deserialize, Serialize, Clone, Debug)]
pub struct ConnectToList {
    #[serde(default)]
    #[garde(skip)]
    /// This is optional because it's used together with [EditQuery] for
    /// searching.
    pub connect_list_id: Option<Uuid>,
}

#[derive(Deserialize, Default)]
pub struct EditQuery {
    #[serde(default)]
    pub search_term: String,
    #[serde(default = "default_search_public_lists")]
    pub search_public_lists: bool,
}

fn default_search_public_lists() -> bool {
    true
}

impl EditQuery {
    pub fn query_string(&self) -> String {
        let mut params = Vec::new();
        if !self.search_term.is_empty() {
            params.push(format!(
                "search_term={}",
                utf8_percent_encode(&self.search_term, NON_ALPHANUMERIC)
            ));
        }
        params.push(format!("search_public_lists={}", self.search_public_lists));
        format!("?{}", params.join("&"))
    }
}
