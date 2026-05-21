use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use fake::Fake;
use itertools::Itertools;
use rand::{
    RngExt,
    seq::{IndexedRandom, IteratorRandom},
};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, AppTx, bookmarks::InsertBookmark},
    forms::{
        ap_users::UpdateApUser,
        links::CreateLink,
        lists::CreateList,
        users::{CreateOidcUser, CreateUser},
    },
};

pub async fn insert_demo_data(
    pool: &PgPool,
    dev_user_credentials: Option<CreateUser>,
    base_url: &Url,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let mut users = create_users(&mut tx, base_url).await?;

    tracing::debug!("Creating dev user...");
    if let Some(create_dev_user) = dev_user_credentials {
        users.push(db::users::create_if_not_exists(&mut tx, create_dev_user, base_url).await?);
    }

    let mut public_lists = Vec::new();
    let mut all_bookmarks = Vec::new();
    let mut public_bookmarks = Vec::new();
    let mut private_lists: HashMap<Uuid, Vec<_>> = HashMap::new();

    tracing::debug!("Creating bookmarks and lists...");
    for user in &users {
        let bookmarks = create_bookmarks(&mut tx, user, base_url).await?;
        all_bookmarks.append(&mut bookmarks.clone());
        let mut lists = Vec::new();

        for _ in 0..100 {
            let content: Option<Vec<_>> = fake::faker::lorem::en::Paragraphs(1..3).fake();
            let title = random_list_name()?;
            let create_list = CreateList {
                title,
                content: content.map(|c| c.join("\n\n")),
                private: fake::Faker.fake(),
            };
            let list = db::lists::insert(&mut tx, user.ap_user_id, create_list).await?;

            if fake::faker::boolean::en::Boolean(10).fake() {
                db::lists::set_pinned(&mut tx, list.id, false).await?;
            }

            if list.private {
                lists.push(list);
            } else {
                public_lists.push(list);
            }
        }

        // Private-to-private links
        for _ in 0..100 {
            let src = lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random list to link from"))?
                .id;
            let dest = random_link_reference(&bookmarks, &lists)?;

            let create_link = CreateLink { src, dest };
            db::links::insert(&mut tx, user.id, create_link).await?;
        }

        private_lists.entry(user.id).or_default().append(&mut lists);
    }

    tracing::debug!("Creating links...");
    for user in users {
        // Public-to-public links
        // Here, we make bookmarks public by linking to them from public lists.
        for _ in 0..1000 {
            let src = public_lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random list to put into a link"))?
                .id;
            let dest = random_link_reference(&all_bookmarks, &public_lists)?;

            let create_link = CreateLink { src, dest };
            let link = db::links::insert(&mut tx, user.id, create_link)
                .await
                .context("Failed to insert link")?;

            if let Some(id) = link.dest_bookmark_id {
                public_bookmarks.push(id);
            }
        }
    }

    // Private-to-public links
    // Here, we can only link to public bookmarks, and private bookmarks from the
    // same user.
    for (user_id, lists) in private_lists {
        // list to public bookmark
        for _ in 0..100 {
            let src = lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random list to link from"))?
                .id;
            let dest = *public_bookmarks
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random bookmark"))?;

            let create_link = CreateLink { src, dest };
            db::links::insert(&mut tx, user_id, create_link).await?;
        }

        // list to public list
        for _ in 0..100 {
            let src = lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random list to link from"))?
                .id;
            let dest = public_lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no public list"))?
                .id;

            let create_link = CreateLink { src, dest };
            db::links::insert(&mut tx, user_id, create_link).await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

async fn create_bookmarks(
    tx: &mut AppTx,
    user: &db::User,
    base_url: &Url,
) -> Result<Vec<db::Bookmark>> {
    let mut bookmarks = Vec::new();

    for _ in 0..500 {
        let tld: String = fake::faker::internet::en::DomainSuffix().fake();
        let word: String = fake::faker::lorem::en::Word().fake();
        let title: String = fake::faker::lorem::en::Words(1..5)
            .fake::<Vec<_>>()
            .join(" ");
        let insert_bookmark = InsertBookmark {
            url: format!("https://{word}.{tld}"),
            title,
        };

        let bookmark =
            db::bookmarks::insert_local(tx, user.ap_user_id, insert_bookmark, base_url).await?;
        bookmarks.push(bookmark);
    }

    Ok(bookmarks)
}

async fn create_users(tx: &mut AppTx, base_url: &Url) -> Result<Vec<db::User>> {
    tracing::debug!("Creating users...");
    let mut users = Vec::new();
    for _ in 0..5 {
        let email: Option<String> = fake::faker::internet::en::SafeEmail().fake();
        let display_name: String = fake::faker::name::en::Name().fake();
        let username = display_name.to_lowercase().replace(' ', "");
        let user = if let Some(email) = email {
            let create_oidc_user = CreateOidcUser {
                oidc_id: Uuid::new_v4().to_string(),
                email,
                username,
            };

            db::users::insert_oidc(tx, create_oidc_user, base_url).await?
        } else {
            let create_user = CreateUser {
                username,
                password: "testpassword".to_string(),
            };

            db::users::insert(tx, create_user, base_url).await?
        };
        let ap_user = db::ap_users::read_by_id(tx, user.ap_user_id).await?;
        users.push(user);

        let bio = fake::faker::lorem::en::Sentence(0..5).fake();

        db::ap_users::update(
            tx,
            ap_user.id,
            UpdateApUser {
                display_name: Some(display_name),
                bio,
            },
        )
        .await?;
    }

    Ok(users)
}

fn random_link_reference(bookmarks: &[db::Bookmark], lists: &[db::List]) -> Result<Uuid> {
    Ok(match rand::rng().random_range(0..=1) {
        0 => {
            bookmarks
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random bookmark to put into a link"))?
                .id
        }
        1 => {
            lists
                .choose(&mut rand::rng())
                .ok_or(anyhow!("Found no random list to put into a link"))?
                .id
        }
        _ => unreachable!(),
    })
}

fn random_list_name() -> Result<String> {
    let adjectives = &[
        "colorful",
        "offline",
        "really good",
        "kinda boring",
        "random",
        "old",
        "archived",
        "fediverse",
        "obscure",
        "vintage",
        "underrated",
        "niche",
        "indie",
        "minimalist",
        "weird",
        "cozy",
        "useful",
        "forgotten",
        "local",
        "experimental",
        "open source",
    ];

    let nouns = &[
        "board games",
        "music",
        "apps",
        "tools",
        "movies",
        "blog posts",
        "reference",
        "tutorials",
        "personal sites",
        "standards",
        "social networks",
        "podcasts",
        "recipes",
        "articles",
        "resources",
        "games",
        "books",
        "zines",
        "newsletters",
        "communities",
        "documentation",
        "fonts",
        "art",
        "photography",
        "web design",
        "comics",
        "videos",
        "research papers",
        "wikis",
        "forums",
    ];

    let chosen_adjectives = adjectives
        .sample(
            &mut rand::rng(),
            (1..3)
                .choose(&mut rand::rng())
                .context("random number required")?,
        )
        .join(" ");

    Ok(format!(
        "{} {}",
        chosen_adjectives,
        nouns
            .iter()
            .choose(&mut rand::rng())
            .context("Noun required")?
    ))
}
