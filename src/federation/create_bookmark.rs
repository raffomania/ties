use activitypub_federation::{
    fetch::object_id::ObjectId,
    kinds::activity::CreateType,
    protocol::verification::{verify_domains_match, verify_is_remote_object},
    traits::{ActivityHandler, Object},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    db, federation,
    response_error::{ResponseError, ResponseResult},
};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateBookmark {
    pub actor: ObjectId<db::ApUser>,
    pub object: federation::Json,
    #[serde(rename = "type")]
    pub kind: CreateType,
    pub id: Url,
}

impl CreateBookmark {
    pub async fn send_to_followers(
        actor: &db::ApUser,
        bookmark: db::Bookmark,
        context: &super::Data,
    ) -> ResponseResult<()> {
        let object = bookmark.into_json(context).await?;
        let id = super::activity::generate_id(context)?;

        let mut tx = context.db_pool.begin().await?;
        let followers = db::ap_users::list_followers(&mut tx, actor.id).await?;
        let create = CreateBookmark {
            actor: actor.ap_id.clone(),
            object,
            kind: CreateType::Create,
            id,
        };

        super::activity::send_with_context(
            actor,
            create,
            super::activity::hashtag_context(),
            &followers.iter().collect::<Vec<_>>(),
            context,
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ActivityHandler for CreateBookmark {
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
        Err(ResponseError::NotFound)
    }
}
