use activitypub_federation::{
    config::Data,
    fetch::object_id::ObjectId,
    kinds::activity::FollowType,
    protocol::verification::{verify_domains_match, verify_is_remote_object},
    traits::ActivityHandler,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    db,
    federation::{self, activity},
    response_error::{ResponseError, ResponseResult},
};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Follow {
    pub actor: ObjectId<db::ApUser>,
    pub object: ObjectId<db::ApUser>,
    #[serde(rename = "type")]
    pub kind: FollowType,
    pub id: Url,
}

impl Follow {
    pub fn new(
        actor: &db::ApUser,
        object: &db::ApUser,
        context: &Data<super::context::Context>,
    ) -> ResponseResult<Self> {
        let id = super::activity::generate_id(context)?;
        let follow = Follow {
            actor: actor.ap_id.clone(),
            object: object.ap_id.clone(),
            kind: FollowType::Follow,
            id,
        };
        Ok(follow)
    }
    pub async fn send(
        self,
        actor: &db::ApUser,
        object: &db::ApUser,
        context: &Data<super::context::Context>,
    ) -> ResponseResult<()> {
        activity::send(actor, self, &[object], context).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ActivityHandler for Follow {
    type DataType = super::Context;
    type Error = ResponseError;

    fn id(&self) -> &Url {
        &self.id
    }

    fn actor(&self) -> &Url {
        self.actor.inner()
    }

    async fn verify(&self, data: &Data<Self::DataType>) -> Result<(), Self::Error> {
        verify_is_remote_object(&self.actor, data)?;
        verify_domains_match(self.actor.inner(), &self.id)?;
        // TODO verify that someone on this server is following the actor
        // https://github.com/raffomania/ties/issues/180
        Ok(())
    }

    async fn receive(self, data: &Data<Self::DataType>) -> Result<(), Self::Error> {
        let actor = self.actor.dereference(data).await?;
        let followed = self.object.dereference_local(data).await?;

        let mut tx = data.db_pool.begin().await?;
        db::follows::upsert(
            &mut tx,
            db::follows::Insert {
                follower_id: actor.id,
                following_id: followed.id,
            },
        )
        .await?;
        tx.commit().await?;

        federation::Accept::send(&followed, self, data).await?;

        Ok(())
    }
}
