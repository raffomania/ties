use std::fmt::Debug;

use activitypub_federation::{
    activity_queue::queue_activity,
    protocol::context::WithContext,
    traits::{ActivityHandler, Actor},
};
use serde::Serialize;
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::{db, federation::context::Data};

pub fn default_context() -> Value {
    json!(["https://www.w3.org/ns/activitystreams"])
}

pub fn hashtag_context() -> Value {
    json!([
        "https://www.w3.org/ns/activitystreams",
        { "Hashtag": "https://www.w3.org/ns/activitystreams#Hashtag" }
    ])
}

pub async fn send<Activity, ActorType: Actor>(
    actor: &ActorType,
    activity: Activity,
    recipients: &[&db::ApUser],
    context: &Data,
) -> Result<(), <Activity as ActivityHandler>::Error>
where
    Activity: ActivityHandler + Serialize + Debug + Send + Sync,
    <Activity as ActivityHandler>::Error: From<activitypub_federation::error::Error>,
{
    send_with_context(actor, activity, default_context(), recipients, context).await
}

pub async fn send_with_context<Activity, ActorType: Actor>(
    actor: &ActorType,
    activity: Activity,
    ap_context: Value,
    recipients: &[&db::ApUser],
    context: &Data,
) -> Result<(), <Activity as ActivityHandler>::Error>
where
    Activity: ActivityHandler + Serialize + Debug + Send + Sync,
    <Activity as ActivityHandler>::Error: From<activitypub_federation::error::Error>,
{
    let activity = WithContext::new(activity, ap_context);
    let inboxes = recipients
        .iter()
        .map(|ap_user| ap_user.shared_inbox_or_inbox())
        .collect();
    queue_activity(&activity, actor, inboxes, context).await?;
    Ok(())
}

pub fn generate_id(context: &Data) -> Result<Url, url::ParseError> {
    context
        .base_url
        .join("/ap/activity/")?
        .join(&Uuid::new_v4().to_string())
}
