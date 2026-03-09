use activitypub_federation::{
    axum::{
        inbox::{ActivityData, receive_activity},
        json::FederationJson,
    },
    config::Data,
    fetch::webfinger::{Webfinger, build_webfinger_response, extract_webfinger_name},
    protocol::context::WithContext,
    traits::{ActivityHandler, Object},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_qs::web::QsQuery;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self},
    extract,
    federation::{self, person::Person},
    response_error::ResponseResult,
    server::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ap/user/{id}", get(get_person))
        .route("/ap/inbox/{user_id}", post(post_inbox))
        .route("/ap/outbox/{user_id}", get(get_outbox))
        .route("/ap/bookmark/{id}", get(get_bookmark))
        .route("/.well-known/webfinger", get(webfinger))
}

/// Read a local person by requesting the URL that is it's `ap_id`.
async fn get_person(
    extract::Tx(mut tx): extract::Tx,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ResponseResult<FederationJson<WithContext<Person>>> {
    let ap_user = db::ap_users::read_by_id(&mut tx, id).await?;
    let json_person = ap_user
        .into_json(&state.federation_config.to_request_data())
        .await?;
    Ok(FederationJson(WithContext::new_default(json_person)))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
#[enum_delegate::implement(ActivityHandler)]
pub enum PersonAcceptedActivities {
    Follow(federation::Follow),
    UndoFollow(federation::UndoFollow),
}

async fn post_inbox(data: federation::Data, activity_data: ActivityData) -> ResponseResult<()> {
    receive_activity::<WithContext<PersonAcceptedActivities>, db::ApUser, federation::Context>(
        activity_data,
        &data,
    )
    .await?;

    Ok(())
}

async fn get_outbox() -> ResponseResult<FederationJson<WithContext<serde_json::Value>>> {
    let empty_outbox = serde_json::json!({
        "type": "OrderedCollection",
        "orderedItems": [],
        "totalItems": 0
    });
    Ok(FederationJson(WithContext::new_default(empty_outbox)))
}

/// Read a local bookmark by requesting the URL that is it's `ap_id`.
async fn get_bookmark(
    extract::Tx(mut tx): extract::Tx,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ResponseResult<FederationJson<WithContext<federation::bookmark::BookmarkJson>>> {
    let bookmark = db::bookmarks::by_id(&mut tx, id).await?;
    let json_bookmark = bookmark
        .into_json(&state.federation_config.to_request_data())
        .await?;
    Ok(FederationJson(WithContext::new_default(json_bookmark)))
}

#[derive(Deserialize)]
pub struct WebfingerQuery {
    resource: String,
}

async fn webfinger(
    extract::Tx(mut tx): extract::Tx,
    QsQuery(query): QsQuery<WebfingerQuery>,
    State(state): State<AppState>,
    data: federation::Data,
) -> ResponseResult<Json<Webfinger>> {
    // This also verifies that the domain is correct
    let username = extract_webfinger_name(&query.resource, &data)?;
    let ap_id = db::ap_users::read_by_username(
        &mut tx,
        federation::webfinger::Resource::from_name_and_url(username.to_string(), &state.base_url)?,
    )
    .await?
    .ap_id;
    Ok(Json(build_webfinger_response(
        query.resource,
        ap_id.into_inner(),
    )))
}
