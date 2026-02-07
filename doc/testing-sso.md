# Testing SSO with Rauthy

1. Run `just start-rauthy` to run [rauthy](https://github.com/sebadob/rauthy) in development mode in a container.
1. Open rauthy in your browser by going to localhost with the port specified by `RAUTHY_PORT` in your `.env` file.
1. Go to the admin area and log in as `admin@rauthy.localhost` with the password `test`.
1. Create a new client. Use `{BASE_URL}/login_oidc_redirect` as your redirect URI, with the base URL defined in your `.env` file. Set access and id algorithm to "EdDSA", if it's not already set.
1. Enter your client ID and secret in your `.env` file.
1. Restart the ties server. Click the  "Sign in with Rauthy" button at the bottom of ties' login page. If it's not there, check the server logs to see if something related to OIDC went wrong.
1. Use the same admin credentials as above to log into rauthy again.
