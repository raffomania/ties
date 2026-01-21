# Plan for Implementing Federation

This document serves as a technical plan for implementing federation in ties, including a survey of how other platforms federate.

This is *not* a standard [FEDERATION.md](https://codeberg.org/fediverse/fep/src/branch/main/fep/67ff/fep-67ff.md) document, as it does not represent ties' current implementation.

## Compatibility

We currently aim for compatibility with Mastodon, Lemmy and Betula. As each of these services has a different feature set, compatibility means a lowest common denominator which both ties and the other service support.

## Users

This is implemented using a pretty standard ActivityPub user object.
Credentials for logging in live in a separate database table from ActivityPub users.

## Lists

We'll probably build something similar to [Lemmy's groups](https://codeberg.org/fediverse/fep/src/branch/main/fep/1b12/fep-1b12.md).
This allows users to receive updates for individual lists - on ties, by adding them to one of their own lists, on mastodon by following the list's actor, and on lemmy by subscribing to the list as a "community".

Private lists are not federated at all. Public lists can not link to private lists. Private lists can link to private lists of the same owner.

Lists have exactly one owner.

## Bookmarks

Bookmarks are considered public if they are added to at least one public list owned by their creator. All other bookmarks are considered private and will not be federated at all, and can only be viewed by their creator.

Betula federates bookmarks [as notes](https://git.sr.ht/~bouncepaw/betula/tree/master/item/fediverse/activities/note.go). The bookmark URL is inserted as an `a` tag into the notes' html body, and as a `Link` object into the `attachments` array.

Lemmy's posts are `Page` objects. It seems like mastodon can ingest both `Note` and `Page` objects as toots, altough pages have their URL appended to the bottom.

Mastodon does not support attachments pointing to websites, only for images, video and audio. This means we'll have to inline the link in the `content` field, like betula does it. Luckily, when reading bookmarks objects from ties, we can just ignore the `content` field and read the attachment instead.

See more at https://docs.joinmastodon.org/spec/activitypub/#status.

## Knowledge Graph

In ties' context, a "knowledge graph" means that connections (also called links or graph edges) can exist between any two of the following objects:

- bookmarks
- notes

On top of this data model, ties can implement lots of functionality:

- note -> note links: Tree of threaded comments
- note -> bookmark links: A list of bookmarks related to a comment / headline
- bookmark -> note links: Comments on a bookmark
- bookmark -> bookmark links: A list of bookmarks related to another bookmark

Users can link objects they didn't author, e.g. Bob can add their own note to Alice's bookmark, or Bob can add Alice's bookmark to their own list. By default, users only see links they created themself, or links of users they follow. This means that there is no single, global view of the whole knowledge graph, but instead each user chooses which part of the knowledge graph they want to view and edit.

Links are directional, and many-to-many: E.g. a comment can have multiple parent and children comments. The knowledge graph is directed and possibly cyclic.

(Tangentially) related FEPs:

- [FEP-e232: Object Links](https://github.com/julianlam/feps/blob/main/fep/e232/fep-e232.md)
    - Uses `tag` to link to other objects
    - deals only with links to activitystreams objects
- [FEP-171b: Conversation Containers](https://codeberg.org/fediverse/fep/src/branch/main/fep/171b/fep-171b.md)
    - Has backfilling
    - not clear if compatible with lemmy / mastodon
- [FEP-dd4b: Quote Posts](https://codeberg.org/fediverse/fep/src/branch/main/fep/dd4b/fep-dd4b.md), [FEP-044f: Consent-respecting quote posts](https://codeberg.org/fediverse/fep/src/branch/main/fep/044f/fep-044f.md)

### Lemmy

Links from a group to its entries can be listed through its `replies` field. Links from an entry to its group are established via an entries' `audience` field. Links from an entry to its parent comment are establisthed via its `inReplyTo` field. This forms a tree structure, which may be adapted to a graph by:

- putting lists into the `inReplyTo` field
- putting a link to a collection into the `inReplyTo` field

After a scan of lemmy's code, it seems like it doesn't currently work with either of these approaches: `inReplyTo` will reject anything that is not a single post or a single comment.

Lemmy's cross-posts are a little similar to ties' links between bookmarks and multiple lists, but they are duplicated in lemmy: each group gets its own post, and they are fetched query time in each read operation.

### Ibis Wiki

Links between wiki pages are stored as custom markdown syntax, e.g. ` 	[[Main_Page@ibis.wiki]]`. It seems like they are not represented via any ActivityStreams objects.
