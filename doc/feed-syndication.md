# Feed syndication in ties

Ties publishes atom feeds for users and individual lists, allowing feed readers to fetch bookmark additions.

Random notes on the implementation:

- Atom format
- Do not include archived content in feed
- strip illegal XML characters that are legal in HTML
- forbid url schemes other than http/https
- use UUIDs for entry ids
- use application/atom+xml;charset=utf8 content type
- only serve public lists and bookmarks over feeds
- feed URLs for private lists return 404
- deleted lists should ideally return `410 Gone` to tell readers about the deletion
- feed URLs should remain stable, try to avoid redirects for renamed resources
- Only use absolute URLs for links inside the feed
