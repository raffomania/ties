use anyhow::Context;
use axum::{
    Router,
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};
use garde::Validate;
use serde::Deserialize;
use serde_qs::web::{QsForm, QsQuery};
use uuid::Uuid;

use crate::{
    authentication::AuthUser,
    db::{self, bookmarks::InsertBookmark},
    extract::{self},
    federation,
    form_errors::FormErrors,
    forms::{self, bookmarks::CreateBookmark, links::CreateLink, lists::CreateList},
    htmf_response::HtmfResponse,
    response_error::{ResponseError, ResponseResult},
    server::AppState,
    views::{self, layout, unsorted_bookmarks},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/bookmarks/create", get(get_create).post(post_create))
        .route("/bookmarks/unsorted", get(get_unsorted))
        .route("/bookmarks/{id}", delete(delete_by_id).get(get_by_id))
        .route("/bookmarks/{id}/edit", get(get_edit))
        .route("/bookmarks/{id}/rename", get(get_edit).post(post_rename))
        .route(
            "/bookmarks/{id}/disconnect",
            get(get_edit).post(post_disconnect),
        )
        .route("/bookmarks/{id}/connect", get(get_edit).post(post_connect))
        .route("/bookmarks/{id}/archive", post(post_archive))
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

    let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
    let is_public = db::bookmarks::is_public(&mut tx, bookmark.id).await?;
    let ap_user = db::ap_users::read_by_id(&mut tx, auth_user.ap_user_id).await?;
    tx.commit().await?;

    if is_public {
        federation::CreateBookmark::send_to_followers(&ap_user, bookmark.clone(), &federation_data)
            .await?;
    }

    state.archive_queue.archive_in_background(archive.id);

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
    QsQuery(query): QsQuery<CreateBookmarkQuery>,
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
            search_results: db::lists::list_recent(&mut tx, auth_user.ap_user_id).await?,
        },
    )))
}

async fn get_edit(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
) -> ResponseResult<HtmfResponse> {
    let loaded = views::edit_bookmark::load(&mut tx, &auth_user, id, search_query).await?;

    if loaded.bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(ResponseError::NotFound);
    }

    Ok(views::edit_bookmark::ViewData { ..loaded.into() }
        .load_search_results(&mut tx, auth_user.ap_user_id)
        .await?
        .view()
        .into())
}

async fn post_rename(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    federation_data: federation::Data,
    Path(id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
    QsForm(rename_input): QsForm<forms::bookmarks::Rename>,
) -> ResponseResult<HtmfResponse> {
    let mut loaded = views::edit_bookmark::load(&mut tx, &auth_user, id, search_query).await?;

    if loaded.bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(ResponseError::NotFound);
    }

    if let Err(errors) = rename_input.validate() {
        let view_data = views::edit_bookmark::ViewData {
            errors: errors.into(),
            rename_input,
            ..loaded.into()
        };
        return Err(ResponseError::InvalidForm(view_data.view().into()));
    }

    loaded.bookmark = db::bookmarks::update_local(
        &mut tx,
        id,
        db::bookmarks::UpdateBookmark {
            title: rename_input.title.clone(),
        },
        auth_user.ap_user_id,
    )
    .await?;

    let is_public = db::bookmarks::is_public(&mut tx, id).await?;
    let ap_user = &db::ap_users::read_by_id(&mut tx, auth_user.ap_user_id).await?;
    let bookmark = loaded.bookmark.clone();

    let data = views::edit_bookmark::ViewData {
        rename_input,
        outcome: views::edit_bookmark::ActionOutcome::Renamed,
        ..loaded.into()
    }
    .load_search_results(&mut tx, auth_user.ap_user_id)
    .await?;

    tx.commit().await?;

    if is_public {
        crate::federation::EditBookmark::send_to_followers(ap_user, bookmark, &federation_data)
            .await?;
    }

    Ok(data.view().into())
}

async fn post_disconnect(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
    QsForm(input): QsForm<forms::bookmarks::Disconnect>,
) -> ResponseResult<HtmfResponse> {
    let mut loaded = views::edit_bookmark::load(&mut tx, &auth_user, id, search_query).await?;

    if loaded.bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(ResponseError::NotFound);
    }

    match db::links::delete_by_id(&mut tx, input.delete_link_id, auth_user.user_id).await {
        // Ignore not found errors, might be caused by a page refresh after deleting a
        // link.
        Err(ResponseError::NotFound) => {}
        result => {
            let link = result?;
            // Make sure the link actually pointed to that bookmark.
            if link.dest_bookmark_id != Some(id) {
                return Err(ResponseError::NotFound);
            }
        }
    }

    loaded
        .connected_lists
        .retain(|link| link.link_id != input.delete_link_id);

    let view_data = views::edit_bookmark::ViewData { ..loaded.into() }
        .load_search_results(&mut tx, auth_user.ap_user_id)
        .await?;

    tx.commit().await?;

    Ok(view_data.view().into())
}

async fn post_connect(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Path(bookmark_id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
    QsForm(input): QsForm<forms::bookmarks::ConnectToList>,
) -> ResponseResult<HtmfResponse> {
    let mut loaded =
        views::edit_bookmark::load(&mut tx, &auth_user, bookmark_id, search_query).await?;

    if loaded.bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(ResponseError::NotFound);
    }

    if let Err(errors) = input.validate() {
        let view_data = views::edit_bookmark::ViewData {
            errors: errors.into(),
            ..loaded.into()
        };

        return Err(ResponseError::InvalidForm(view_data.view().into()));
    }

    if let Some(src) = input.connect_list_id {
        let target_list = db::lists::by_id(&mut tx, src).await?;

        // Only allow linking to own lists
        if target_list.ap_user_id != auth_user.ap_user_id {
            return Err(ResponseError::NotFound);
        }

        let link = db::links::insert(
            &mut tx,
            auth_user.user_id,
            forms::links::CreateLink {
                src,
                dest: bookmark_id,
            },
        )
        .await?;

        loaded
            .connected_lists
            .push(views::edit_bookmark::LinkWithList {
                link_id: link.id,
                list_title: target_list.title,
                list_private: target_list.private,
            });
    }

    let data = views::edit_bookmark::ViewData { ..loaded.into() }
        .load_search_results(&mut tx, auth_user.ap_user_id)
        .await?;

    tx.commit().await?;

    Ok(data.view().into())
}

async fn get_by_id(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    let bookmark = db::bookmarks::by_id(&mut tx, id).await?;

    if !db::bookmarks::is_public(&mut tx, bookmark.id).await?
        && bookmark.ap_user_id != auth_user.ap_user_id
    {
        return Err(ResponseError::NotFound);
    }

    let archive = db::archives::by_bookmark_id(&mut tx, bookmark.id).await?;
    let backlinks = db::lists::pointing_to_bookmark(
        &mut tx,
        id,
        layout.authed_info.as_ref().map(|a| a.ap_user_id),
    )
    .await?;
    let username = db::ap_users::read_by_id(&mut tx, bookmark.ap_user_id)
        .await?
        .username;

    Ok(HtmfResponse(views::show_bookmark::view(
        views::show_bookmark::Data {
            layout,
            bookmark,
            archive,
            backlinks,
            username,
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

async fn post_archive(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ResponseResult<Redirect> {
    let bookmark = db::bookmarks::by_id(&mut tx, id).await?;
    if bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(crate::response_error::ResponseError::NotFound);
    }

    db::archives::delete_by_bookmark_id(&mut tx, id).await?;

    let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
    tx.commit().await?;

    state.archive_queue.archive_in_background(archive.id);

    Ok(Redirect::to(&format!("/bookmarks/{id}")))
}
