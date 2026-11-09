//! Atom syndication format representation of a bookmark.
//!
//! See [RFC 4287] for details about the format.
//!
//! [RFC 4287]: https://www.rfc-editor.org/rfc/rfc4287

use anyhow::Result;
use quick_xml::se::Serializer as XmlSerializer;
use serde::{Serialize, Serializer};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::{
    db::{self, Bookmark},
    views::common::bookmark_title,
};

/// XML namespace of the Atom syndication format.
pub static ATOM_NAMESPACE: &str = "http://www.w3.org/2005/Atom";

/// Permanent, universally unique identifier of a feed or entry.
///
/// Serialized as a [`urn:uuid`] IRI, which stays valid even when the
/// server's base URL changes.
///
/// [`urn:uuid`]: https://www.rfc-editor.org/rfc/rfc8141
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
    /// Links to the feed itself, and its corresponding web page
    #[serde(rename = "link")]
    pub links: Vec<Link>,
    pub author: Person,
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

/// Used mainly to describe authors of a feed or its entries.
#[derive(Debug, Clone, Serialize)]
pub struct Person {
    name: String,
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
        let mut serializer = XmlSerializer::with_root(&mut document, Some("feed"))?;
        serializer.indent(' ', 2);
        self.serialize(serializer)?;
        remove_illegal_xml_chars(&mut document);
        Ok(document)
    }

    pub fn for_list(
        db::ListWithMetadata { list, metadata }: &db::ListWithMetadata,
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
            author: Person {
                name: metadata.username.clone(),
            },
            links: vec![
                Link {
                    href: base_url.join(&list.feed_path())?,
                    rel: "self",
                },
                Link {
                    href: base_url.join(&list.path())?,
                    rel: "alternate",
                },
            ],
            entries,
        })
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
                // Primary "external link" shown by feed readers
                Link {
                    rel: "alternate",
                    href: bookmark_url,
                },
                // Secondary "external link", shown only by some feed readers
                Link {
                    rel: "via",
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
                rel: "alternate",
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
    use pretty_assertions::assert_eq;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::{Content, Feed};
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
            created_at: OffsetDateTime::UNIX_EPOCH,
            ap_user_id: Uuid::new_v4(),
            title: "list title".to_string(),
            content: None,
            private: false,
            pinned: false,
        };
        let metadata = db::lists::Metadata {
            linked_bookmark_count: bookmarks.len().try_into()?,
            linked_list_count: 0,
            username: "username".to_string(),
        };
        let list_with_metadata = db::ListWithMetadata { list, metadata };
        Feed::for_list(&list_with_metadata, bookmarks, &"https://ties.pub".parse()?)
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

        Ok(())
    }

    #[test]
    fn falls_back_to_generic_title() -> anyhow::Result<()> {
        let feed = feed_for(&[bookmark_link(None, "https://ties.pub/")?])?;

        assert_eq!(feed.entries[0].title, "Untitled Bookmark");

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
        assert!(!xml.contains("<script>"));

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
        let feed = feed_for(&[bookmark_link(Some("Example"), "https://ties.pub/")?])?;

        assert_eq!(feed.xmlns, "http://www.w3.org/2005/Atom");
        assert!(
            feed.to_xml()?
                .contains(r#"xmlns="http://www.w3.org/2005/Atom""#)
        );

        Ok(())
    }
}
