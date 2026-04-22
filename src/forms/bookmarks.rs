use garde::Validate;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use uuid::Uuid;

use crate::{db::bookmarks::InsertBookmark, form_errors::FormErrors};

#[derive(Validate, Default, Deserialize, Clone, Debug)]
pub struct CreateBookmark {
    #[garde(skip)]
    #[serde(default)]
    pub parents: Vec<Uuid>,

    #[garde(skip)]
    #[serde(default)]
    pub create_parents: Vec<String>,

    #[garde(url)]
    pub url: String,

    #[garde(length(min = 1, max = 500))]
    pub title: String,

    #[garde(length(max = 100))]
    pub list_search_term: Option<String>,

    #[garde(skip)]
    #[serde(default)]
    pub submitted: bool,
}

impl TryFrom<CreateBookmark> for InsertBookmark {
    type Error = FormErrors;

    fn try_from(value: CreateBookmark) -> Result<Self, Self::Error> {
        value.validate()?;

        if !value.submitted {
            return Err(FormErrors::default());
        }

        Ok(InsertBookmark {
            url: value.url,
            title: value.title,
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

#[derive(Validate, Default, Deserialize, Clone, Debug)]
pub struct ConnectToList {
    #[serde(default)]
    #[garde(skip)]
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
