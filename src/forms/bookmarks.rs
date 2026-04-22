use garde::Validate;
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

    #[garde(custom(not_empty))]
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

#[expect(clippy::trivially_copy_pass_by_ref)]
fn not_empty(value: &str, _: &()) -> garde::Result {
    if value.is_empty() {
        Err(garde::Error::new("cannot be empty"))
    } else {
        Ok(())
    }
}
