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
    forms::{self, bookmarks::CreateBookmark},
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
        .route("/bookmarks/{id}/archive-title", get(get_archive_title))
}

/// Create a private bookmark with an empty title and redirect the user to the
/// edit screen.
async fn post_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    State(state): State<AppState>,
    QsForm(input): QsForm<CreateBookmark>,
) -> ResponseResult<Response> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    let insert_bookmark = match InsertBookmark::try_from(input.clone()) {
        Err(errors) => {
            return Ok(HtmfResponse(views::create_bookmark::view(
                &views::create_bookmark::Data {
                    layout,
                    errors,
                    input,
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

    let archive = db::archives::insert_pending(&mut tx, bookmark.id).await?;
    tx.commit().await?;

    state.archive_queue.archive_in_background(archive.id);

    Ok(Redirect::to(&bookmark.edit_path()).into_response())
}

#[derive(Deserialize)]
struct CreateBookmarkQuery {
    url: Option<String>,
}

async fn get_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    QsQuery(query): QsQuery<CreateBookmarkQuery>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    Ok(HtmfResponse(views::create_bookmark::view(
        &views::create_bookmark::Data {
            layout,
            errors: FormErrors::default(),
            input: CreateBookmark {
                url: query.url.unwrap_or_default(),
            },
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
            title: Some(rename_input.title.clone()),
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

    // Since this is intended to be used in the bookmark edit form only,
    // and that form is only intended to work on your own bookmarks, we can be
    // more restrictive here than technically necessary.
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
    federation_data: federation::Data,

    Path(bookmark_id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
    QsForm(input): QsForm<forms::bookmarks::ConnectToList>,
) -> ResponseResult<HtmfResponse> {
    let mut loaded =
        views::edit_bookmark::load(&mut tx, &auth_user, bookmark_id, search_query).await?;

    // Since this is intended to be used in the bookmark edit form only,
    // and that form is only intended to work on your own bookmarks, we can be
    // more restrictive here than technically necessary.
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

    let bookmark_public_before = db::bookmarks::is_public(&mut tx, bookmark_id).await?;

    if let Some(src) = input.connect_list_id {
        let target_list = db::lists::by_id(&mut tx, src).await?;

        let link = db::links::insert(
            &mut tx,
            auth_user.user_id,
            auth_user.ap_user_id,
            forms::links::CreateLink {
                src,
                dest: bookmark_id,
            },
        )
        .await?;

        let bookmark_public_after = !target_list.private;

        if !bookmark_public_before && bookmark_public_after {
            let ap_user = db::ap_users::read_by_id(&mut tx, loaded.bookmark.ap_user_id).await?;
            federation::CreateBookmark::send_to_followers(
                &ap_user,
                loaded.bookmark.clone(),
                &federation_data,
            )
            .await?;
        }

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

    if !db::bookmarks::is_public_or_owner(&mut tx, auth_user.ap_user_id, bookmark.id).await? {
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
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ResponseResult<HeaderMap> {
    db::bookmarks::delete_by_id(&mut tx, id, auth_user.ap_user_id).await?;

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

async fn get_archive_title(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    QsQuery(search_query): QsQuery<forms::bookmarks::EditQuery>,
) -> ResponseResult<HtmfResponse> {
    let bookmark = db::bookmarks::by_id(&mut tx, id).await?;

    if bookmark.ap_user_id != auth_user.ap_user_id {
        return Err(ResponseError::NotFound);
    }

    let title_from_archive = db::archives::title(&mut tx, id, auth_user.ap_user_id).await?;

    Ok(HtmfResponse(views::edit_bookmark::archive_title_view(
        &title_from_archive,
        bookmark.title.as_deref(),
        bookmark.id,
        &search_query,
    )))
}
