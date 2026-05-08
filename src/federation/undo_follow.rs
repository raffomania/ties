use activitypub_federation::{
    fetch::object_id::ObjectId,
    kinds::activity::UndoType,
    protocol::{
        helpers::deserialize_skip_error,
        verification::{verify_is_remote_object, verify_urls_match},
    },
    traits::ActivityHandler,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    db,
    federation::{self, activity},
    response_error::{ResponseError, ResponseResult},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoFollow {
    pub actor: ObjectId<db::ApUser>,
    /// For compatibility with platforms that always expect to receive the
    /// recipient field
    #[serde(deserialize_with = "deserialize_skip_error", default)]
    pub to: Option<[ObjectId<db::ApUser>; 1]>,
    pub object: federation::Follow,
    #[serde(rename = "type")]
    pub kind: UndoType,
    pub id: Url,
}

impl UndoFollow {
    pub async fn send(
        actor: &db::ApUser,
        object: federation::Follow,
        context: &federation::Data,
    ) -> ResponseResult<()> {
        let id = super::activity::generate_id(context)?;
        let undo_follow = UndoFollow {
            actor: actor.ap_id.clone(),
            to: Some([object.object.clone()]),
            object: object.clone(),
            kind: UndoType::Undo,
            id,
        };
        activity::send(
            actor,
            undo_follow,
            &[&object.object.dereference(context).await?],
            context,
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ActivityHandler for UndoFollow {
    type DataType = super::Context;
    type Error = ResponseError;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, data: &federation::Data) -> Result<(), Self::Error> {
        verify_urls_match(self.actor.inner(), self.object.actor.inner())?;
        if let Some(to) = &self.to {
            verify_urls_match(to[0].inner(), self.object.object.inner())?;
        }
        verify_is_remote_object(&self.actor, data)?;
        self.object.verify(data).await?;
        Ok(())
    }

    async fn receive(self, data: &federation::Data) -> Result<(), Self::Error> {
        let follower = self.actor.dereference(data).await?;
        let following = self.object.object.dereference(data).await?;

        let mut tx = data.db_pool.begin().await?;
        db::follows::remove(
            &mut tx,
            db::follows::Insert {
                follower_id: follower.id,
                following_id: following.id,
            },
        )
        .await?;
        tx.commit().await?;

        Ok(())
    }
}
