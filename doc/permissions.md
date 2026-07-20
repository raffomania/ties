# Permissions in Ties

## Lists

Lists are the central unit of organisation in ties.
They are explicitly marked as either public or private.

- Private lists are only visible to their owner and are not federated.
- Public lists are visible for anyone, and are listed on a user's profile.
- Only the list owner can change a list in any way. This includes connecting/disconnecting items (create/delete links), renaming the list, pinning or unpinning it, or toggling its public/private status.

## Bookmarks

Bookmarks are considered private by default.
Once they are added to at least one public list, they are considered public.

- Private bookmarks are only visible to their owner and are not federated.
- Public bookmarks are visible for anyone, including their archived content.
- Global search shows only a user's own bookmarks at the moment, but might include public bookmarks by other users in the future.
- Only the bookmark owner can change a bookmark in any way, such as editing its title or deleting it.

## Links

What the UI calls "connecting" and "disconnecting", or "adding" and "removing" items from lists, the codebase calls creating or deleting "links".
A link points from its "source" to its "destination" item.
The source is always a list, the destination can be a list or a bookmark.

- Only the list owner can create links pointing from it to other items.
- Links to public items are visible for anyone. Links to private items are only visible for the item's owner.
- Links to and from private items may only be created by the items' owner.
- Links can only be changed or deleted by their owner.
