# ties Changelog

## Unreleased

linkblocks is now named **ties**!

### Breaking Changes

- The container is now at `ghcr.io/raffomania/ties`.
- For development environments, if you want to use the new recommended development URL of `ties.localhost`, you'll have to update `BASE_URL` in your `.env` file, `rm -r ./development_cert` and run `just development cert`.

### Highlights

- Lists now have a "Backlinks" section at the top, allowing you to quickly navigate through your knowledge graph.
- Search through bookmark titles using the search bar at the top of every page.

### Bugfixes

- Fix missing spaces around some labels in the UI ([#206](https://github.com/raffomania/ties/issues/206))
- Fix the incorrect link to the page for installing the bookmarklet by moving the installation instructions to the start page.

### Docs

- Mention the `latest` tag in the deployment guide.
- In the deployment guide and CLI help, mention that it's not supported to change the `BASE_URL` once accounts have been created.

### Internals

- Update all dependencies.
- Make error handling more robust for unauthenticated requests that need to get redirected to login ([#204](https://github.com/raffomania/ties/pull/204), thanks @danilax86!)

## 0.1.0

_Released on 2025-11-23_

This is the initial release of linkblocks!
A lot of groundwork has been laid for federating with other services, and posting bookmarks to Mastodon is the first fruit of that labor available with this release. For an example, check out [rafael@lb.rafa.ee](https://mstdn.io/@rafael@lb.rafa.ee), or try it with [the linkblocks demo](https://linkblocks.rafa.ee).

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
