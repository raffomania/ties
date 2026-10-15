//! Atom syndication format representation of a bookmark.
//!
//! See [RFC 4287] for details about the format.
//!
//! [RFC 4287]: https://www.rfc-editor.org/rfc/rfc4287

use anyhow::Result;
use axum::{
    http::header,
    response::{IntoResponse, Response},
};
use quick_xml::se::to_writer_with_root;
use serde::{Serialize, Serializer};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, Bookmark},
    response_error::ResponseError,
    views::content::bookmark_title,
};

/// XML namespace of the Atom syndication format.
pub static ATOM_NAMESPACE: &str = "http://www.w3.org/2005/Atom";

/// Permanent, universally unique identifier of a feed or entry.
///
/// Serialized as a [`urn:uuid`] IRI, which stays valid even when the
/// server's base URL changes. See [RFC 4287] for the requirements on
/// `atom:id`.
///
/// [`urn:uuid`]: https://www.rfc-editor.org/rfc/rfc8141
/// [RFC 4287]: https://www.rfc-editor.org/rfc/rfc4287#section-4.2.6
#[derive(Debug, Clone)]
pub struct AtomId(pub Uuid);

impl Serialize for AtomId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("urn:uuid:{}", self.0))
    }
}

/// A feed of bookmarks.
///
/// Serialized to an Atom feed document with quick-xml.
#[derive(Debug, Clone, Serialize)]
pub struct Feed {
    /// XML namespace, always [`ATOM_NAMESPACE`].
    #[serde(rename = "@xmlns")]
    pub xmlns: &'static str,
    /// A permanent, universally unique identifier for the feed.
    pub id: AtomId,
    /// Human-readable title of the feed.
    pub title: String,
    /// Time of the most recent change to any bookmark in the feed.
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    /// Link to the web page this feed represents.
    pub link: Link,
    /// The bookmarks in the feed, serialized as repeated `<entry>` elements
    /// as required by the Atom spec.
    #[serde(rename = "entry")]
    pub entries: Vec<Entry>,
}

/// A single bookmark in a feed.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// A permanent, universally unique identifier for the bookmark.
    pub id: AtomId,
    /// Human-readable title of the bookmark, as entered by the user.
    pub title: String,
    /// Time of the most recent change to the bookmark.
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    /// Links to the bookmarked web page and to the bookmark's page hosted by
    /// ties.
    #[serde(rename = "link")]
    pub links: Vec<Link>,
    /// Content shown in the feed reader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,
}

/// A link to a web page related to the feed or entry it belongs to.
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    /// Relationship of the linked web page, e.g. `alternate`.
    #[serde(rename = "@rel")]
    pub rel: &'static str,
    /// Address of the linked web page, as entered by the user.
    #[serde(rename = "@href")]
    pub href: Url,
}

/// Textual content of a bookmark.
#[derive(Debug, Clone, Serialize)]
pub struct Content {
    /// MIME type of the content, e.g. `text` or `html`.
    #[serde(rename = "@type")]
    pub type_: &'static str,
    /// The content itself.
    #[serde(rename = "$text")]
    pub text: String,
}

impl Feed {
    /// Serialize the feed into an Atom XML document. Removes characters that
    /// are illegal in XML.
    pub fn to_xml(&self) -> Result<String> {
        let mut document = String::from(r#"<?xml version="1.0" encoding="utf-8"?>"#);
        to_writer_with_root(&mut document, "feed", self)?;
        remove_illegal_xml_chars(&mut document);
        Ok(document)
    }

    pub fn for_list(
        list: &db::List,
        links: &[db::LinkWithContent],
        base_url: &Url,
    ) -> Result<Self> {
        let updated = links
            .iter()
            .map(|l| l.created_at())
            .max()
            .unwrap_or(list.created_at);

        let entries = links
            .iter()
            .map(|l| Entry::from_link(l, base_url))
            .filter_map(|e| e.ok())
            .collect();
        Ok(Feed {
            xmlns: ATOM_NAMESPACE,
            id: AtomId(list.id),
            title: list.title.clone(),
            updated,
            link: Link {
                href: base_url.join(&list.path())?,
                rel: "self",
            },
            entries,
        })
    }
}

impl IntoResponse for Feed {
    fn into_response(self) -> Response {
        match self.to_xml() {
            Ok(document) => (
                [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
                document,
            )
                .into_response(),
            Err(error) => ResponseError::from(error).into_response(),
        }
    }
}

/// Remove characters that are not allowed in XML documents from `text`.
///
/// User-defined input like bookmark titles can contain characters such as
/// NUL which can't be represented in XML at all. Keeping them would produce
/// an invalid document, so they are removed instead. Legal whitespace like
/// tabs and line breaks is kept.
///
/// See the `Char` production in the [XML specification] for the characters
/// that are allowed in XML documents.
///
/// [XML specification]: https://www.w3.org/TR/xml/#charsets
fn remove_illegal_xml_chars(text: &mut String) {
    /// Whether `character` is allowed in XML documents.
    fn is_legal_xml_char(character: char) -> bool {
        matches!(
            character,
            // Whitespace
            '\u{9}' | '\u{A}' | '\u{D}'
            // Characters that aren't assigned to anything, like
            // `U+0000` and other C0 control characters
            | '\u{20}'..='\u{D7FF}'
            // Excludes the non-characters `U+FFFE` and `U+FFFF`
            | '\u{E000}'..='\u{FFFD}'
            | '\u{10000}'..='\u{10FFFF}'
        )
    }

    text.retain(is_legal_xml_char);
}

impl Entry {
    fn from_bookmark(bookmark: &Bookmark, base_url: &Url) -> Result<Self> {
        let bookmark_url = Url::parse(&bookmark.url)?;
        Ok(Self {
            id: AtomId(bookmark.id),
            title: bookmark_title(&bookmark.title).to_string(),
            updated: bookmark.created_at,
            links: vec![
                Link {
                    rel: "alternate",
                    href: bookmark_url,
                },
                Link {
                    rel: "self",
                    href: base_url.join(&bookmark.show_path())?,
                },
            ],
            content: None,
        })
    }

    fn from_list(list: &db::List, base_url: &Url) -> Result<Self> {
        Ok(Self {
            id: AtomId(list.id),
            title: list.title.clone(),
            updated: list.created_at,
            links: vec![Link {
                rel: "self",
                href: base_url.join(&list.path())?,
            }],
            content: None,
        })
    }

    fn from_link(link: &db::LinkWithContent, base_url: &Url) -> Result<Self> {
        match &link.dest {
            db::LinkDestinationWithMetadata::Bookmark(bookmark) => {
                Self::from_bookmark(bookmark, base_url)
            }
            db::LinkDestinationWithMetadata::List(list_with_metadata) => {
                Self::from_list(&list_with_metadata.list, base_url)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use activitypub_federation::fetch::object_id::ObjectId;
    use anyhow::Result;
    use axum::{
        body::to_bytes,
        http::{HeaderValue, StatusCode, header},
        response::IntoResponse,
    };
    use pretty_assertions::assert_eq;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::{ATOM_NAMESPACE, AtomId, Content, Feed, Link};
    use crate::db::{self, bookmarks::Bookmark};

    /// A bookmark with the user-defined `title` and `url` as only variable
    /// parts. The `id` is used as the last part of the bookmark's
    /// ActivityPub id.
    fn bookmark_link(title: Option<&str>, url: &str) -> anyhow::Result<db::LinkWithContent> {
        let id = Uuid::from_u128(0x1234);
        Ok(db::LinkWithContent {
            id: Uuid::new_v4(),
            dest: db::links::LinkDestinationWithMetadata::Bookmark(Bookmark {
                id,
                created_at: OffsetDateTime::UNIX_EPOCH,
                ap_user_id: Uuid::from_u128(0x4321),
                url: url.to_string(),
                title: title.map(str::to_string),
                ap_id: format!("https://ties.pub/ap/bookmark/{id}")
                    .parse::<ObjectId<Bookmark>>()?,
            }),
        })
    }

    fn feed_for(bookmarks: &[db::LinkWithContent]) -> Result<Feed> {
        let list = db::List {
            id: Uuid::from_u128(0x1235),
            created_at: OffsetDateTime::now_utc(),
            ap_user_id: Uuid::new_v4(),
            title: "list title".to_string(),
            content: None,
            private: false,
            pinned: false,
        };
        Feed::for_list(&list, bookmarks, &"https://ties.pub".parse()?)
    }

    #[test]
    fn serializes_bookmark_as_feed() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(Some("Example"), "https://ties.pub/")?])?;

        insta::assert_snapshot!(feed.to_xml()?);

        Ok(())
    }

    #[test]
    fn serializes_every_entry() -> anyhow::Result<()> {
        let feed = feed_for(&[
            bookmark_link(Some("First"), "https://example.com/1")?,
            bookmark_link(Some("Second"), "https://example.com/2")?,
        ])?;
        let xml = feed.to_xml()?;

        assert_eq!(xml.matches("<entry>").count(), 2);
        assert!(xml.find("<title>First</title>") < xml.find("<title>Second</title>"));

        Ok(())
    }

    #[test]
    fn falls_back_to_generic_title() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(None, "https://ties.pub/")?])?;

        assert!(
            feed.entries
                .iter()
                .all(|entry| entry.title == "Untitled Bookmark")
        );

        Ok(())
    }

    #[test]
    fn escapes_user_defined_title() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(
            Some(r#"<script>alert("xss & fun")</script> > 1 & 1"#),
            "https://ties.pub/",
        )?])?;

        let xml = feed.to_xml()?;

        assert!(xml.contains(
            r#"<title>&lt;script&gt;alert("xss &amp; fun")&lt;/script&gt; &gt; 1 &amp; 1</title>"#
        ));
        // The user-defined title cannot break out of the title element.
        assert!(!xml.contains("<script>"));
        assert!(!xml.contains("</title><script"));

        Ok(())
    }

    #[test]
    fn escapes_user_defined_url_in_link_attribute() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(
            Some("Example"),
            r#"https://ties.pub/?a=1&b="2"><script>alert("xss")</script>"#,
        )?])?;

        let xml = feed.to_xml()?;

        dbg!(&xml);
        assert!(xml.contains(
            r#"href="https://ties.pub/?a=1&amp;b=%222%22%3E%3Cscript%3Ealert(%22xss%22)%3C/script%3E""#
        ));
        assert!(!xml.contains(r#""><script>"#));
        assert!(!xml.contains("b=\"2\""));

        Ok(())
    }

    #[test]
    fn leaves_content_empty() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(Some("Example"), "https://ties.pub/")?])?;

        assert!(feed.entries.iter().all(|entry| entry.content.is_none()));
        assert!(!feed.to_xml()?.contains("<content"));

        Ok(())
    }

    #[test]
    fn escapes_user_defined_content() -> anyhow::Result<()> {
        let mut feed = feed_for(&[bookmark_link(Some("Example"), "https://ties.pub/")?])?;
        for entry in &mut feed.entries {
            entry.content = Some(Content {
                type_: "text",
                text: r#"</content><script>alert("xss")</script>"#.to_string(),
            });
        }

        let xml = feed.to_xml()?;

        assert!(xml.contains(
            r#"<content type="text">&lt;/content&gt;&lt;script&gt;alert("xss")&lt;/script&gt;</content>"#
        ));
        assert!(!xml.contains("<script>"));

        Ok(())
    }

    /// Illegal XML characters in user-defined input are removed from the
    /// serialized output.
    #[test]
    fn removes_illegal_xml_characters() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(
            Some("Ex\u{0}am\u{1}ple\u{fffe}"),
            "https://ties.pub/\u{0}\u{1f}path",
        )?])?;

        let xml = feed.to_xml()?;

        // The generated xml does not contain any of the illegal input.
        for illegal in ['\u{0}', '\u{1}', '\u{1f}', '\u{fffe}'] {
            assert!(
                !xml.contains(illegal),
                "XML contains illegal character {illegal:?}: {xml}"
            );
        }

        // The title is kept without its illegal characters.
        assert!(xml.contains("<title>Example</title>"));

        Ok(())
    }

    #[test]
    fn feed_uses_the_atom_namespace() -> anyhow::Result<()> {
        let feed = Feed {
            xmlns: ATOM_NAMESPACE,
            id: AtomId(Uuid::new_v4()),
            title: "Example".to_string(),
            updated: OffsetDateTime::UNIX_EPOCH,
            link: Link {
                rel: "alternate",
                href: "https://ties.pub/".parse()?,
            },
            entries: Vec::new(),
        };

        assert_eq!(feed.xmlns, "http://www.w3.org/2005/Atom");
        assert!(
            feed.to_xml()?
                .contains(r#"xmlns="http://www.w3.org/2005/Atom""#)
        );

        Ok(())
    }

    /// The feed is served as an Atom document that looks exactly like
    /// [`Feed::to_xml`] produces it.
    #[tokio::test]
    async fn responds_with_atom_xml() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(Some("Example"), "https://ties.pub/")?])?;
        let expected_body = feed.to_xml()?;
        let response = feed.into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&HeaderValue::from_static(
                "application/atom+xml; charset=utf-8"
            ))
        );

        let body = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(std::str::from_utf8(&body)?, expected_body);

        Ok(())
    }
}
