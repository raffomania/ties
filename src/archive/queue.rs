//! In-memory queue for processing archival requests in the background with rate
//! limiting.
//!
//! - Jobs are lost when the server restarts.
//! - Jobs are automatically queued at the top of the queue when a new local
//!   bookmark is inserted.
//! - Failed extraction will not be retried automatically.
//! - Bookmarks that have no archive and no previously failed archival attempt
//!   will be enqueued automatically. The database stores archival status for
//!   each bookmark (ok, not archived yet, failed).
//! - Jobs are processed in serial, without parallelism.
//! - Requests are rate limited globally.

use anyhow::Result;
use tokio::sync::{mpsc, oneshot};

use crate::{
    archive,
    db::{self, Bookmark},
};

struct Queue {
    receiver: tokio::sync::mpsc::Receiver<Message>,
    db_pool: sqlx::PgPool,
}

enum Message {
    ArchiveBookmark {
        bookmark: db::Bookmark,
        respond_to: oneshot::Sender<Result<db::Archive>>,
    },
}

impl Queue {
    fn new(receiver: mpsc::Receiver<Message>, db_pool: sqlx::PgPool) -> Self {
        Self { receiver, db_pool }
    }

    async fn process(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            self.handle_message(msg).await;
        }
    }

    async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ArchiveBookmark {
                bookmark,
                respond_to,
            } => self.archive_bookmark(bookmark, respond_to).await,
        }
    }

    async fn archive_bookmark(
        &mut self,
        bookmark: Bookmark,
        respond_to: oneshot::Sender<Result<db::Archive>>,
    ) {
        tracing::info!(?bookmark, "Archiving bookmark");
        let article = self.get_article(&bookmark).await;
        let archive = self.save_archive(&bookmark, &article).await;

        tracing::info!(?bookmark, "Archived bookmark");

        let _ = respond_to.send(archive);
    }

    async fn get_article(&mut self, bookmark: &Bookmark) -> Result<legible::Article> {
        let html = archive::fetch_url_as_text(&bookmark.url).await?;
        tracing::debug!(html_length = html.len(), "Fetched website HTML");
        let article = archive::make_readable(bookmark.url.parse()?, &html)?;
        tracing::debug!(
            readable_html_length = article.content.len(),
            "Extracted readable HTML"
        );

        Ok(article)
    }

    async fn save_archive(
        &mut self,
        bookmark: &Bookmark,
        article: &Result<legible::Article>,
    ) -> Result<db::Archive> {
        let mut tx = self.db_pool.begin().await?;

        let archive = db::archives::insert(&mut tx, bookmark.id, article).await?;

        tx.commit().await?;

        Ok(archive)
    }
}

#[derive(Clone)]
pub struct QueueHandle {
    sender: mpsc::Sender<Message>,
}

impl QueueHandle {
    pub fn new(db_pool: sqlx::PgPool) -> Self {
        let (sender, receiver) = mpsc::channel(10);

        let queue = Queue::new(receiver, db_pool);
        tokio::spawn(queue.process());

        Self { sender }
    }

    /// Dispatch a bookmark for archiving, but ignore any failures.
    pub fn archive_bookmark(&self, bookmark: Bookmark) {
        let (send, _recv) = oneshot::channel();
        let msg = Message::ArchiveBookmark {
            bookmark,
            respond_to: send,
        };

        let _ = self.sender.try_send(msg);
    }
}
