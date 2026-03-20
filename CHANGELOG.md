# ties Changelog

*Note: This file does not contain any LLM-generated text.*

## Unreleased

linkblocks is now named **ties**!

### Breaking Changes / Operation Notes

- The container is now at `ghcr.io/raffomania/ties`.
- For development environments, if you want to use the new recommended development URL of `ties.localhost`, you'll have to update `BASE_URL` in your `.env` file, `rm -r ./development_cert` and run `just development cert`.
- The backend will now make requests to external hosts based on user input. [SSRF](https://owasp.org/www-community/attacks/Server_Side_Request_Forgery) protection is in place. Nonetheless, please consider how this may interact with your server-side networking setup, and make sure the ties backend does not have network access to private resources.

### 📚 Bookmark Archiving

Bookmarked websites are now fetched, converted into a readable version, and saved in the database. Click a bookmark headline to view its archived text.
Media and other resources such as images, styles or scripts are not archived.

When you update to this release and start the server, all your existing bookmarks will be automatically archived in the background.

### 🔎 Bookmark Search

Search through bookmark titles, URLs and archived text content using the search bar at the top of every page.

### 🔗 Backlinks

Lists now have a "Backlinks" section at the top, allowing you to quickly navigate through your knowledge graph.

### Features

- Add total bookmark and list count to index page.
- Move logout button to index page to make sidebar less noisy.
- Add the ties logo to the login and index pages, and add a favicon.
- Use a darker background for page header sections to distinguish them from pages' main content.
- Add the ties version and a link to the source code at the bottom of the start page.
- Options in the CLI help are now grouped by category.
- There's a new `version` CLI command for printing the version of ties you're running.

### Bugfixes

- Fix missing spaces around some labels in the UI ([#206](https://github.com/raffomania/ties/issues/206))
- Fix the incorrect link to the page for installing the bookmarklet by moving the installation instructions to the start page.
- Fix demo mode not deleting data from all tables.
- Remove old container before building new one in `just build-podman-container` task.

### Docs

- Mention the `latest` tag in the deployment guide.
- In the deployment guide and CLI help, mention that it's not supported to change the `BASE_URL` once accounts have been created.
- Add a contribution guide.

### Internals

- Update all dependencies.
- Make error handling more robust for unauthenticated requests that need to get redirected to login ([#204](https://github.com/raffomania/ties/pull/204), thanks @danilax86!)
- Add live reloading in development.
- Decrease the time it takes to reload the server in watch mode.
- Check formatting in CI.
- Disable htmf formatter and return unformatted HTML in debug mode as well. The formatter introduced whitespace changes that resulted in rendering differences between debug and release modes.
- Replace our own `serde_qs` query extractor wrapper with the one provided by `serde_qs`.
- Install CA Certificates in the container to allow making TLS network requests.

## 0.1.0

_Released on 2025-11-23_

This is the initial release of linkblocks!
A lot of groundwork has been laid for federating with other services, and posting bookmarks to Mastodon is the first fruit of that labor available with this release. For an example, check out [rafael@ties.rafa.ee](https://mstdn.io/@rafael@ties.rafa.ee), or try it with [the linkblocks demo](https://linkblocks.rafa.ee).

linkblocks is now quite stable, and I've been using it for myself for over a year.
Of course there are still some rough edges, and tons of features I'd like to add, so watch this space!

### Features

- Post bookmarks to Mastodon: any bookmark added to a public list is considered public and will show up in the timeline.
- Look up linkblocks user handles via webfinger. This should work on most fediverse platforms, and was tested with Lemmy.
- See all public lists of a user on the new profile page.
- Organize bookmarks using lists with arbitrary nesting.
- Single-sign-on: Register and log in via OIDC.
- Add new bookmarks with a single click using the bookmarklet.
- Deploy it as a single binary, with PostgreSQL as the only dependency.
