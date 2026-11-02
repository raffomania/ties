use sqlx::query;

use super::util::test_app::TestApp;
use crate::{
    db::{self, LinkDestinationWithMetadata},
    forms::{self, lists::CreateList},
};

/// Look up what list_by_list reports to `viewer` about the number of lists that
/// `list` links to.
///
/// Returns `None` when the link itself is hidden from `viewer`.
async fn nested_list_count(
    tx: &mut db::AppTx,
    list_id: uuid::Uuid,
    viewer: uuid::Uuid,
) -> anyhow::Result<Vec<i64>> {
    let links = db::links::list_by_list(tx, list_id, Some(viewer)).await?;
    let counts = links
        .iter()
        .map(|link| match &link.dest {
            LinkDestinationWithMetadata::List(l) => l.metadata.linked_list_count,
            LinkDestinationWithMetadata::Bookmark(_) => {
                panic!("expected a list destination")
            }
        })
        .collect::<Vec<_>>();
    Ok(counts)
}

/// When someone opens a list, its page shows how many other lists it links to.
/// That number is built by `db::links::list_by_list`.
///
/// This test checks what the number does when a list links to a private list
/// that belongs to a different user. A link to a private list may only be seen
/// by the owner of that private list, no matter who owns the list the link
/// starts from. So the link to a private list should only be counted for the
/// owner of that private list.
#[test_log::test(tokio::test)]
async fn list_by_list_nested_counts_of_private_lists() -> anyhow::Result<()> {
    let app = TestApp::new().await;
    let lists_owner = app.create_user("lists_owner", "longpassword").await;
    let private_list_owner = app.create_user("private_list_owner", "longpassword").await;

    let mut tx = app.tx().await;

    // lists_owner owns viewed_list (public), the list we look at
    let viewed_list = db::lists::insert(
        &mut tx,
        lists_owner.ap_user_id,
        CreateList {
            title: "viewed_list".to_string(),
            content: None,
            private: false,
        },
    )
    .await?;
    // lists_owner owns linked_list (public), the list that viewed_list links
    // to
    let linked_list = db::lists::insert(
        &mut tx,
        lists_owner.ap_user_id,
        CreateList {
            title: "linked_list".to_string(),
            content: None,
            private: false,
        },
    )
    .await?;
    // private_list_owner owns private_list (private), the list that
    // linked_list links to
    let private_list = db::lists::insert(
        &mut tx,
        private_list_owner.ap_user_id,
        CreateList {
            title: "private_list".to_string(),
            content: None,
            private: true,
        },
    )
    .await?;

    // Link viewed_list to linked_list
    db::links::insert(
        &mut tx,
        lists_owner.id,
        lists_owner.ap_user_id,
        forms::links::CreateLink {
            src: viewed_list.id,
            dest: linked_list.id,
        },
    )
    .await?;

    // Link linked_list to private_list. This is not allowed by
    // db::links::insert so we do a manual query.
    query(r"insert into links (user_id, src_list_id, dest_list_id) values ($1, $2, $3)")
        .bind(lists_owner.id)
        .bind(linked_list.id)
        .bind(private_list.id)
        .execute(&mut *tx)
        .await?;

    // Result for lists_owner, who owns linked_list, but not the private list it
    // links to. The link to private_list is only visible to its owner, so it
    // is not counted here. The link to the public linked_list itself is still
    // shown.
    assert_eq!(
        nested_list_count(&mut tx, viewed_list.id, lists_owner.ap_user_id)
            .await?
            .as_slice(),
        [0]
    );

    // Result for private_list_owner, who owns the private list that linked_list
    // links to. That link is visible to them and counted here. The link to
    // linked_list is shown to them as well.
    assert_eq!(
        nested_list_count(&mut tx, viewed_list.id, private_list_owner.ap_user_id)
            .await?
            .as_slice(),
        [1]
    );

    Ok(())
}
