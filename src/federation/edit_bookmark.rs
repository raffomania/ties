use activitypub_federation::{
    fetch::object_id::ObjectId,
    kinds::activity,
    protocol::{
        helpers::deserialize_one_or_many,
        verification::{verify_domains_match, verify_is_remote_object},
    },
    traits::{ActivityHandler, Object},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::{
    db, federation,
    response_error::{ResponseError, ResponseResult},
};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditBookmark {
    pub actor: ObjectId<db::ApUser>,
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub to: Vec<Url>,
    pub object: federation::bookmark::Json,
    #[serde(rename = "type")]
    pub kind: activity::UpdateType,
    pub id: Url,
}

impl EditBookmark {
    pub async fn send_to_followers(
        actor: &db::ApUser,
        bookmark: db::Bookmark,
        context: &super::Data,
    ) -> ResponseResult<()> {
        let mut object = bookmark.into_json(context).await?;
        // Required by mastodon to be a newer timestamp than the last value
        object.updated = Some(OffsetDateTime::now_utc());

        let id = super::activity::generate_id(context)?;

        let mut tx = context.db_pool.begin().await?;
        let followers = db::ap_users::list_followers(&mut tx, actor.id).await?;
        let to = followers
            .iter()
            .map(|ap_user| ap_user.ap_id.clone().into_inner())
            .collect();
        let edit = EditBookmark {
            actor: actor.ap_id.clone(),
            to,
            object,
            kind: activity::UpdateType::Update,
            id,
        };

        super::activity::send(actor, edit, &followers.iter().collect::<Vec<_>>(), context).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ActivityHandler for EditBookmark {
    type DataType = super::context::Context;
    type Error = ResponseError;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, data: &super::Data) -> Result<(), Self::Error> {
        verify_is_remote_object(&self.actor, data)?;
        verify_domains_match(self.actor.inner(), self.object.id.inner())?;
        db::Bookmark::verify(&self.object, self.actor.inner(), data).await?;

        Ok(())
    }

    async fn receive(self, _data: &super::Data) -> Result<(), Self::Error> {
        // Not yet implemented.
        Err(ResponseError::NotFound)
    }
}
