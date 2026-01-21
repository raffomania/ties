use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::{link::LinkType, object::NoteType, public},
    protocol::verification::{verify_domains_match, verify_is_remote_object},
    traits::Object,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    db::{self, bookmarks::InsertBookmark},
    response_error::{ResponseError, into_option},
};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkJson {
    pub id: ObjectId<db::Bookmark>,
    #[serde(rename = "type")]
    pub kind: NoteType,
    pub attributed_to: ObjectId<db::ApUser>,
    pub to: Vec<Url>,
    /// Formatted content with the url inlined, for platforms that don't support
    /// link attachments
    pub content: Option<String>,
    /// The title
    pub name: Option<String>,
    #[serde(default)]
    pub(crate) attachments: Vec<Link>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Link {
    href: String,
    media_type: Option<String>,
    #[serde(rename = "type")]
    kind: LinkType,
}

impl TryFrom<BookmarkJson> for InsertBookmark {
    type Error = anyhow::Error;

    fn try_from(value: BookmarkJson) -> Result<Self, Self::Error> {
        let first_attachment = value.attachments.first();
        let url = if let Some(attachment) = first_attachment.cloned() {
            Some(attachment.href)
        } else {
            None
        }
        .ok_or_else(|| anyhow!("Missing URL"))?;

        let create_bookmark = InsertBookmark {
            url,
            title: value.name.ok_or_else(|| anyhow!("Missing title"))?,
        };

        // TODO how to validate InsertBookmark?
        // ideally, we'd find a single point for the validation rules that
        // now live in CreateBookmark
        // https://github.com/raffomania/ties/issues/163

        Ok(create_bookmark)
    }
}

#[async_trait::async_trait]
impl Object for db::Bookmark {
    type DataType = super::Context;
    type Kind = BookmarkJson;
    type Error = ResponseError;

    async fn read_from_id(
        url: Url,
        data: &Data<Self::DataType>,
    ) -> Result<Option<Self>, Self::Error> {
        let mut tx = data.db_pool.begin().await?;
        let bookmark = db::bookmarks::by_ap_id(&mut tx, ObjectId::from(url)).await;
        into_option(bookmark)
    }

    async fn into_json(self, data: &Data<Self::DataType>) -> Result<Self::Kind, Self::Error> {
        let mut tx = data.db_pool.begin().await?;
        let author = db::ap_users::read_by_id(&mut tx, self.ap_user_id).await?;
        let attachments = vec![Link {
            href: self.url.clone(),
            // TODO according to ActivityStreams, this "identifies the MIME media type of the
            // referenced resource", but we currently do not fetch remote URLs so we
            // have no way of knowing the media type
            // https://github.com/raffomania/ties/issues/164
            media_type: None,
            kind: LinkType::Link,
        }];
        let content = format!(
            r#"<p>{}</p><a href="{}">{}</p>"#,
            self.title, self.url, self.url
        );
        Ok(BookmarkJson {
            id: self.ap_id,
            kind: NoteType::Note,
            attributed_to: author.ap_id,
            to: vec![public()],
            content: Some(content),
            name: Some(self.title),
            attachments,
        })
    }

    async fn verify(
        json: &Self::Kind,
        expected_domain: &Url,
        data: &Data<Self::DataType>,
    ) -> Result<(), Self::Error> {
        verify_domains_match(json.id.inner(), expected_domain)?;
        verify_is_remote_object(&json.id, data)?;
        // TODO see validation todo above
        // InsertBookmark::try_from(json.clone())?.validate()?;
        // https://github.com/raffomania/ties/issues/163
        Ok(())
    }

    async fn from_json(json: Self::Kind, data: &Data<Self::DataType>) -> Result<Self, Self::Error> {
        let ap_user = json.attributed_to.dereference(data).await?;
        let mut tx = data.db_pool.begin().await?;
        let ap_id = json.id.clone();
        let insert_bookmark = json.try_into()?;
        let new_bookmark =
            db::bookmarks::upsert_remote(&mut tx, ap_user.id, &ap_id, insert_bookmark).await?;
        tx.commit().await?;
        Ok(new_bookmark)
    }
}
