use anyhow::Context;
use axum::{
    Router,
    extract::{Path, Query},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get},
};
use garde::Validate;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    authentication::AuthUser,
    db::{self, LinkDestination},
    extract::{self, qs_form::QsForm},
    form_errors::FormErrors,
    forms::links::{CreateLink, PartialCreateLink},
    htmf_response::HtmfResponse,
    response_error::ResponseResult,
    server::AppState,
    views::{self, layout},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/links/create", get(get_create).post(post_create))
        .route("/links/{id}", delete(delete_by_id))
}

async fn post_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    // TODO handle failed extractors in forms better
    QsForm(input): QsForm<PartialCreateLink>,
) -> ResponseResult<Response> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;
    let src_from_db = match input.src {
        Some(id) => Some(db::items::by_id(&mut tx, id).await?),
        None => None,
    };
    let dest_from_db = match input.dest {
        Some(id) => Some(db::items::by_id(&mut tx, id).await?),
        None => None,
    };

    if let Err(errors) = input.validate() {
        return Ok(
            HtmfResponse(views::create_link::view(views::create_link::Data {
                layout,
                errors: errors.into(),
                form_input: input,
                search_results: Vec::new(),
                src_from_db,
                dest_from_db,
            }))
            .into_response(),
        );
    }

    let search_term = match (input.src, input.dest) {
        (None, _) => input.search_term_src.as_ref(),
        (Some(_), None) => input.search_term_dest.as_ref(),
        _ => None,
    };

    // TODO exclude items that are already linked
    // TODO: if source is public, only show public destinations
    // if source is private, only show private destinations from the same owner
    // https://github.com/raffomania/ties/issues/149
    let search_results = match search_term {
        Some(search_term) => db::lists::search(&mut tx, search_term, auth_user.ap_user_id).await?,
        None => Vec::new(),
    };

    // TODO if the link points to a public list and we haven't sent an activity for
    // this src bookmark before, send an activity now
    // https://github.com/raffomania/ties/issues/175
    if let (Some(src), Some(dest), true) = (&src_from_db, &dest_from_db, input.submitted) {
        db::links::insert(
            &mut tx,
            auth_user.user_id,
            CreateLink {
                src: src.id(),
                dest: dest.id(),
            },
        )
        .await?;

        tx.commit().await?;

        return Ok(Redirect::to(&src.path()).into_response());
    }

    Ok(
        HtmfResponse(views::create_link::view(views::create_link::Data {
            layout,
            errors: FormErrors::default(),
            form_input: input,
            search_results,
            src_from_db,
            dest_from_db,
        }))
        .into_response(),
    )
}

#[derive(Deserialize)]
struct CreateLinkQueryString {
    src_id: Option<Uuid>,
    dest_id: Option<Uuid>,
}

async fn get_create(
    extract::Tx(mut tx): extract::Tx,
    auth_user: AuthUser,
    Query(query): Query<CreateLinkQueryString>,
) -> ResponseResult<HtmfResponse> {
    let layout = layout::Template::from_db(&mut tx, Some(&auth_user)).await?;

    let src = match query.src_id {
        Some(id) => Some(db::items::by_id(&mut tx, id).await?),
        _ => None,
    };

    let dest = match query.dest_id {
        Some(id) => Some(db::items::by_id(&mut tx, id).await?),
        _ => None,
    };

    // TODO exclude items that are already linked
    let search_results = match (src.as_ref(), dest.as_ref()) {
        (None, _) | (_, None) => db::lists::list_recent(&mut tx, auth_user.ap_user_id).await?,
        _ => Vec::new(),
    };

    Ok(views::create_link::view(views::create_link::Data {
        layout,
        errors: FormErrors::default(),
        form_input: PartialCreateLink {
            src: src.as_ref().map(LinkDestination::id),
            dest: dest.as_ref().map(LinkDestination::id),
            ..PartialCreateLink::default()
        },
        search_results,
        src_from_db: src,
        dest_from_db: dest,
    })
    .into())
}

async fn delete_by_id(
    extract::Tx(mut tx): extract::Tx,
    Path(id): Path<Uuid>,
) -> ResponseResult<HeaderMap> {
    db::links::delete_by_id(&mut tx, id).await?;

    tx.commit().await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Refresh",
        "true".parse().context("Failed to parse header value")?,
    );

    Ok(headers)
}
