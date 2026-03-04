//! In-memory queue for processing archival requests in the background with rate
//! limiting.
//!
//! - Jobs are automatically queued when a new local bookmark is inserted.
//! - Failed extraction will not be retried automatically.
//! - The database stores archival status for each bookmark (ok, not archived
//!   yet, failed).
//! - Jobs are processed in serial, without parallelism.
//! - Requests are not rate limited at the moment.

use anyhow::Result;
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

use crate::{
    archive,
    db::{self, AppTx},
};

struct Queue {
    receiver: tokio::sync::mpsc::Receiver<Message>,
    db_pool: sqlx::PgPool,
    processed_archive_id_sender: broadcast::Sender<Uuid>,
}

struct ArchiveTask {
    archive_id: Uuid,
    respond_to: oneshot::Sender<Result<db::Archive>>,
}

enum Message {
    ArchiveBookmark(ArchiveTask),
}

impl Queue {
    fn new(
        receiver: mpsc::Receiver<Message>,
        db_pool: sqlx::PgPool,
        processed_archive_id_sender: broadcast::Sender<Uuid>,
    ) -> Self {
        Self {
            receiver,
            db_pool,
            processed_archive_id_sender,
        }
    }

    async fn process(mut self) {
        tracing::debug!("Starting");

        while !self.receiver.is_closed() {
            // Process all incoming messages with high priority.
            while let Ok(message) = self.receiver.try_recv() {
                self.receive_message(message).await;
            }

            tracing::debug!("Processed all high-priority tasks");

            // No incoming messages at the moment, process an arbitrary pending bookmark if
            // it exists.
            let pending_archive_id = self.get_pending_archive_id().await;

            if let Some(pending_archive_id) = pending_archive_id {
                tracing::debug!("Found dangling pending archive");
                let _ = self.archive(pending_archive_id).await;
            }

            // No work at the moment: wait for new messages to come in
            if self.receiver.is_empty() && pending_archive_id.is_none() {
                tracing::debug!("No archiving to do, waiting...");

                if let Some(msg) = self.receiver.recv().await {
                    self.receive_message(msg).await;
                }
            }
        }

        tracing::debug!("Exiting");
    }

    async fn receive_message(&mut self, message: Message) {
        tracing::debug!("Received message");
        match message {
            Message::ArchiveBookmark(task) => {
                let archive = self.archive(task.archive_id).await;

                let _ = task.respond_to.send(archive);
            }
        }
    }

    async fn get_pending_archive_id(&mut self) -> Option<Uuid> {
        let mut tx = self.db_pool.begin().await.ok()?;
        let pending_id = db::archives::find_one_pending(&mut tx).await.ok()?;
        let _ = tx.commit().await;

        pending_id
    }

    async fn archive(&mut self, archive_id: Uuid) -> Result<db::Archive> {
        let mut tx = self.db_pool.begin().await?;
        let pending = db::archives::by_id(&mut tx, archive_id).await?;
        let bookmark = db::bookmarks::by_id(&mut tx, pending.bookmark_id).await?;

        tracing::info!(?bookmark, "Archiving bookmark");
        let article = self.get_article(&bookmark.url).await;
        if article.is_err() {
            tracing::info!(?article, "Fetching complete");
        }
        let archive = self.save_archive(&mut tx, &pending, &article).await;
        match &archive {
            Ok(archive) => {
                let _ = self.processed_archive_id_sender.send(archive.id);
            }
            Err(_) => {
                tracing::error!(?archive, "Could not save archive");
            }
        }

        tx.commit().await?;

        tracing::info!(?archive, "Archived bookmark");

        archive
    }

    async fn get_article(&mut self, url: &str) -> Result<legible::Article, archive::Error> {
        let html = archive::fetch_url_as_text(url).await?;
        tracing::debug!(html_length = html.len(), "Fetched website HTML");
        let article = archive::make_readable(url.parse()?, &html)?;
        tracing::debug!(
            readable_html_length = article.content.len(),
            "Extracted readable HTML"
        );

        Ok(article)
    }

    async fn save_archive(
        &mut self,
        tx: &mut AppTx,
        archive: &db::Archive,
        article: &std::result::Result<legible::Article, archive::Error>,
    ) -> Result<db::Archive> {
        let archive = db::archives::update(tx, archive.id, article).await?;

        Ok(archive)
    }
}

#[derive(Clone)]
pub struct QueueHandle {
    sender: mpsc::Sender<Message>,
    // Intentionally do not store a receiver here: there's no associated process that receives
    // messages, so having a receiver here would clog up the channel
    processed_archive_id_sender: broadcast::Sender<Uuid>,
}

impl QueueHandle {
    pub fn new(db_pool: sqlx::PgPool) -> Self {
        let (sender, receiver) = mpsc::channel(50);

        // Since the messages here are only UUIDs, we can afford a large buffer size to
        // support slow receivers.
        let (processed_archive_id_sender, _) = broadcast::channel(200);

        let queue = Queue::new(receiver, db_pool, processed_archive_id_sender.clone());
        tokio::spawn(queue.process());

        Self {
            sender,
            processed_archive_id_sender,
        }
    }

    /// Dispatch a bookmark for archiving, but ignore any failures.
    pub fn archive_in_background(&self, archive_id: Uuid) {
        let (send, _recv) = oneshot::channel();
        let msg = Message::ArchiveBookmark(ArchiveTask {
            archive_id,
            respond_to: send,
        });

        let _ = self.sender.try_send(msg);
    }

    /// Wait for the given archive id to be processed.
    pub async fn wait_until_archive_processed(&self, archive_id: Uuid) -> bool {
        let mut receiver = self.processed_archive_id_sender.subscribe();

        // If the receiver leaks or is closed, quit
        while let Ok(processed_id) = receiver.recv().await {
            if processed_id == archive_id {
                return true;
            }
        }

        false
    }
}
