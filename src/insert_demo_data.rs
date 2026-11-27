use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use fake::Fake;
use itertools::Itertools;
use rand::{
    RngExt,
    seq::{IndexedRandom, IteratorRandom},
};
use sqlx::PgPool;
use tokio::task::JoinSet;
use url::Url;
use uuid::Uuid;

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
struct UserRef {
    user_id: Uuid,
    ap_user_id: Uuid,
}

use crate::{
    archive,
    authentication::hash_password,
    db::{self, AppTx, bookmarks::InsertBookmark},
    forms::{
        ap_users::UpdateApUser,
        links::CreateLink,
        lists::CreateList,
        users::{CreateOidcUser, CreateUser},
    },
};

struct UserData {
    user_ref: UserRef,
    bookmarks: Vec<db::Bookmark>,
    private_lists: Vec<db::List>,
    public_lists: Vec<db::List>,
}

const DEMO_USER_PASSWORD: &str = "demo-password";

pub async fn insert_demo_data(
    pool: &PgPool,
    dev_user_credentials: Option<CreateUser>,
    base_url: &Url,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let mut users = create_users(&mut tx, base_url).await?;

    tracing::debug!("Creating dev user...");
    if let Some(create_dev_user) = dev_user_credentials {
        users.push(
            db::users::create_root_admin_if_not_exists(&mut tx, create_dev_user, base_url).await?,
        );
    }
    tx.commit().await?;

    tracing::debug!("Creating bookmarks and lists...");
    let mut tasks = JoinSet::new();
    for user in users {
        let pool = pool.clone();
        let base_url = base_url.clone();
        tasks.spawn(async move { create_user_data(&pool, &user, &base_url).await });
    }
    let user_data = join_tasks(tasks).await?;

    let all_public_lists = user_data
        .iter()
        .flat_map(|data| data.public_lists.iter().cloned())
        .collect::<Arc<[_]>>();

    tracing::debug!("Creating public links...");
    let mut tasks = JoinSet::new();
    for data in user_data {
        let pool = pool.clone();
        let all_public_lists = Arc::clone(&all_public_lists);
        tasks.spawn(async move { create_public_links(&pool, data, all_public_lists).await });
    }
    let user_data = join_tasks(tasks).await?;

    let all_public_bookmarks = user_data
        .iter()
        .flat_map(|(_, bookmarks)| bookmarks.iter().copied())
        .collect::<Arc<[_]>>();
    let all_public_lists = user_data
        .iter()
        .map(|(data, _)| data.public_lists.clone())
        .collect::<Arc<[_]>>();

    tracing::debug!("Creating private-to-public links...");
    let mut tasks = JoinSet::new();
    for (data, _) in user_data {
        let pool = pool.clone();
        let all_public_bookmarks = Arc::clone(&all_public_bookmarks);
        let all_public_lists = Arc::clone(&all_public_lists);
        tasks.spawn(async move {
            create_private_links(&pool, data, all_public_bookmarks, all_public_lists).await
        });
    }
    join_tasks(tasks).await?;

    Ok(())
}

async fn join_tasks<T: Send + 'static>(mut tasks: JoinSet<Result<T>>) -> Result<Vec<T>> {
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result.context("Demo data task failed")??);
    }
    Ok(results)
}

async fn create_user_data(pool: &PgPool, user: &db::User, base_url: &Url) -> Result<UserData> {
    let mut tx = pool.begin().await?;
    let bookmarks = create_bookmarks(&mut tx, user, base_url).await?;
    let mut private_lists = Vec::new();
    let mut public_lists = Vec::new();

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
            private_lists.push(list);
        } else {
            public_lists.push(list);
        }
    }

    // Private-to-private links
    for _ in 0..100 {
        let src = private_lists
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no random list to link from"))?
            .id;
        let dest = random_link_reference(&bookmarks, &private_lists)?;

        let create_link = CreateLink { src, dest };
        db::links::insert(&mut tx, user.id, user.ap_user_id, create_link).await?;
    }

    tx.commit().await?;

    Ok(UserData {
        user_ref: UserRef {
            user_id: user.id,
            ap_user_id: user.ap_user_id,
        },
        bookmarks,
        private_lists,
        public_lists,
    })
}

async fn create_public_links(
    pool: &PgPool,
    data: UserData,
    all_public_lists: Arc<[db::List]>,
) -> Result<(UserData, Vec<Uuid>)> {
    let mut tx = pool.begin().await?;
    let mut public_bookmarks = Vec::new();

    // Public-to-public links
    // Here, we make bookmarks public by linking to them from public lists.
    for _ in 0..1000 {
        let src = data
            .public_lists
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no random list to put into a link"))?
            .id;
        let dest = random_link_reference(&data.bookmarks, &all_public_lists)?;

        let create_link = CreateLink { src, dest };
        let link = db::links::insert(
            &mut tx,
            data.user_ref.user_id,
            data.user_ref.ap_user_id,
            create_link,
        )
        .await
        .context("Failed to insert link")?;

        if let Some(id) = link.dest_bookmark_id {
            public_bookmarks.push(id);
        }
    }

    tx.commit().await?;

    Ok((data, public_bookmarks))
}

async fn create_private_links(
    pool: &PgPool,
    data: UserData,
    all_public_bookmarks: Arc<[Uuid]>,
    all_public_lists: Arc<[Vec<db::List>]>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let UserData {
        user_ref,
        private_lists: lists,
        ..
    } = data;

    // Private-to-public links
    // Here, we can only link to public bookmarks, and private bookmarks from
    // the same user.
    // list to public bookmark
    for _ in 0..100 {
        let src = lists
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no random list to link from"))?
            .id;
        let dest = *all_public_bookmarks
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no random bookmark"))?;

        let create_link = CreateLink { src, dest };
        db::links::insert(&mut tx, user_ref.user_id, user_ref.ap_user_id, create_link).await?;
    }

    // list to public list
    for _ in 0..100 {
        let src = lists
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no random list to link from"))?
            .id;
        let dest = all_public_lists
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no public list"))?
            .choose(&mut rand::rng())
            .ok_or(anyhow!("Found no public list"))?
            .id;

        let create_link = CreateLink { src, dest };
        db::links::insert(&mut tx, user_ref.user_id, user_ref.ap_user_id, create_link).await?;
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
        let title = rand::random_bool(0.95).then(|| {
            fake::faker::lorem::en::Words(1..5)
                .fake::<Vec<_>>()
                .join(" ")
        });
        let insert_bookmark = InsertBookmark {
            url: random_url()?,
            title,
        };

        let bookmark =
            db::bookmarks::insert_local(tx, user.ap_user_id, insert_bookmark, base_url).await?;
        create_archive(tx, &bookmark).await?;
        bookmarks.push(bookmark);
    }

    Ok(bookmarks)
}

fn random_url() -> Result<String> {
    [
        "https://www.rafa.ee",
        "https://github.com",
        "https://wikipedia.org",
        "https://duckduckgo.com",
    ]
    .choose(&mut rand::rng())
    .map(ToString::to_string)
    .context("Found no random URL")
}

async fn create_archive(tx: &mut AppTx, bookmark: &db::Bookmark) -> Result<db::Archive> {
    let archive = db::archives::insert_pending(tx, bookmark.id).await?;

    let article = if rand::random_bool(0.9) {
        Ok(random_article(
            bookmark.title.clone(),
            bookmark.url.clone(),
        )?)
    } else {
        // TODO derive fake::Dummy on archive::Error to get random errors here
        // https://github.com/raffomania/ties/issues/333
        Err(archive::Error::NotReadable)
    };

    let archive = db::archives::update(tx, archive.id, &article).await?;

    Ok(archive)
}

fn random_article(title: Option<String>, url: String) -> Result<legible::Article> {
    let content: Vec<_> = fake::faker::lorem::en::Paragraphs(1..100).fake();

    Ok(legible::Article {
        title: title.unwrap_or_default(),
        byline: fake::faker::name::en::Name().fake(),
        dir: None,
        lang: Some("en".to_string()),
        // TODO count graphemes instead
        // https://github.com/raffomania/ties/issues/333
        length: content.len(),
        excerpt: Some(
            content
                .first()
                .context("No first paragraph for random article")?
                .clone(),
        ),
        text_content: content.clone().join("\n\n"),
        content: content.iter().map(|p| format!("<p>{p}</p>")).join("\n"),
        site_name: Some(url),
        // TODO use random date
        // https://github.com/raffomania/ties/issues/333
        published_time: None,
        markdown_content: content.into_iter().join("\n\n"),
    })
}

async fn create_users(tx: &mut AppTx, base_url: &Url) -> Result<Vec<db::User>> {
    tracing::debug!("Creating users...");
    let mut password_hash: Option<String> = None;
    let mut users: Vec<db::User> = Vec::new();
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
            let invited_by = users.choose(&mut rand::rng()).map(|u| u.id);
            let password_hash = if let Some(hash) = &password_hash {
                hash.clone()
            } else {
                let hash = hash_password(DEMO_USER_PASSWORD)?;
                password_hash = Some(hash.clone());
                hash
            };

            db::users::insert_with_password_hash(tx, username, password_hash, invited_by, base_url)
                .await?
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
