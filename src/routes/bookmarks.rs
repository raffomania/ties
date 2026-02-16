use anyhow::Context;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    authentication::AuthUser,
    db::{self, bookmarks::InsertBookmark},
    extract::{self, qs_form::QsForm},
    federation,
    form_errors::FormErrors,
    forms::{bookmarks::CreateBookmark, links::CreateLink, lists::CreateList},
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views::{self, layout, unsorted_bookmarks},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/bookmarks/create", get(get_create).post(post_create))
        .route("/bookmarks/unsorted", get(get_unsorted))
        .route("/bookmarks/{id}", delete(delete_by_id))
}

async fn post_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
    federation_data: federation::Data,
    QsForm(input): QsForm<CreateBookmark>,
) -> ResponseResult<Response> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    let selected_parents = db::lists::list_by_id(&mut tx, &input.parents).await?;

    // TODO exclude items that are already linked
    let search_results = match input.list_search_term.as_ref() {
        None => db::lists::list_recent(&mut tx, auth_user.ap_user_id).await?,
        Some(term) => db::lists::search(&mut tx, term, auth_user.ap_user_id).await?,
    };

    let insert_bookmark = match InsertBookmark::try_from(input.clone()) {
        Err(errors) => {
            return Ok(HtmfResponse(views::create_bookmark::view(
                &views::create_bookmark::Data {
                    layout,
                    errors,
                    input,
                    selected_parents,
                    search_results,
                },
            ))
            .into_response());
        }
        Ok(i) => i,
    };

    let bookmark = db::bookmarks::insert_local(
        &mut tx,
        auth_user.ap_user_id,
        insert_bookmark,
        &state.base_url,
    )
    .await?;

    let mut first_created_parent = Option::None;
    for parent_title in input.create_parents {
        let parent = db::lists::insert(
            &mut tx,
            auth_user.ap_user_id,
            CreateList {
                title: parent_title,
                content: None,
                private: false,
            },
        )
        .await?;
        db::links::insert(
            &mut tx,
            auth_user.user_id,
            CreateLink {
                src: parent.id,
                dest: bookmark.id,
            },
        )
        .await?;

        if first_created_parent.is_none() {
            first_created_parent.replace(parent);
        }
    }

    for parent in input.parents {
        db::links::insert(
            &mut tx,
            auth_user.user_id,
            CreateLink {
                src: parent,
                dest: bookmark.id,
            },
        )
        .await?;
    }

    if db::bookmarks::is_public(&mut tx, bookmark.id).await? {
        federation::CreateBookmark::send_to_followers(
            &db::ap_users::read_by_id(&mut tx, auth_user.ap_user_id).await?,
            bookmark.clone(),
            &federation_data,
        )
        .await?;
    }

    tx.commit().await?;

    // Start archiving bookmark but don't wait for completion
    state.archive_queue.archive_bookmark(bookmark);

    let redirect_dest = match selected_parents.first().or(first_created_parent.as_ref()) {
        Some(parent) => parent.path(),
        None => "/bookmarks/unsorted".to_string(),
    };
    Ok(Redirect::to(&redirect_dest).into_response())
}

#[derive(Deserialize)]
struct CreateBookmarkQuery {
    parent_id: Option<Uuid>,
    url: Option<String>,
    title: Option<String>,
}

async fn get_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Query(query): Query<CreateBookmarkQuery>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    let selected_parent = match query.parent_id {
        Some(id) => Some(db::lists::by_id(&mut tx, id).await?),
        _ => None,
    };

    Ok(HtmfResponse(views::create_bookmark::view(
        &views::create_bookmark::Data {
            layout,
            errors: FormErrors::default(),
            input: CreateBookmark {
                parents: Vec::new(),
                url: query.url.unwrap_or_default(),
                title: query.title.unwrap_or_default(),
                ..Default::default()
            },
            selected_parents: selected_parent.into_iter().collect(),
            // TODO exclude items that are already linked
            search_results: db::lists::list_recent(&mut tx, auth_user.user_id).await?,
        },
    )))
}

async fn get_unsorted(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;
    let bookmarks = db::bookmarks::list_unsorted(&mut tx, auth_user.ap_user_id).await?;

    Ok(HtmfResponse(unsorted_bookmarks::view(
        &unsorted_bookmarks::Data { layout, bookmarks },
    )))
}

async fn delete_by_id(
    extract::Tx(mut tx): extract::Tx,
    Path(id): Path<Uuid>,
) -> ResponseResult<HeaderMap> {
    db::bookmarks::delete_by_id(&mut tx, id).await?;

    tx.commit().await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Refresh",
        "true".parse().context("Failed to parse header value")?,
    );

    Ok(headers)
}
