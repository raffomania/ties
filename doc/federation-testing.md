# Testing federation with other software manually

## Using public instances

The easiest way to test federation features with Mastodon is to expose a local development environment to a public domain, e.g. using one of these tools:

- ngrok
- serveo
- localhost.run

When using this approach, **do not** run the `insert-demo-data` script, as it will add users with unsafe passwords that could lead to your environment getting compromised.

1. Wipe your local database using `just wipe-database`.
1. Run the forwarding tool of your choice, forwarding the port specified in `.env`.
1. Put the public domain you're assigned into `BASE_URL` in `.env`. Depending on your forwarding tool, you might need to turn off TLS.
1. Run ties using `just run` or `just watch`.

You can now use any Mastodon instance to interact with your local ties instance, e.g. by pasting the handle to your local ties user (`@username@domain`).

For debugging, we recommend https://activitypub.academy, a Mastodon instance with one-click signup and extra ActivityPub tools.
