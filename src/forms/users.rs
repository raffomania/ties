use garde::Validate;
use openidconnect::{AuthorizationCode, CsrfToken};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Validate)]
pub struct CreateUser {
    #[garde(pattern("^[a-zA-Z0-9_]+$"), length(min = 3, max = 50))]
    pub username: String,
    #[garde(length(min = 10, max = 100))]
    pub password: String,
}

#[derive(Validate, Default, Deserialize, Debug)]
pub struct OidcSelectUsername {
    #[garde(pattern("^[a-zA-Z0-9_]+$"), ascii, length(min = 3, max = 50))]
    pub username: String,
}

#[derive(Serialize, Deserialize, Validate, Debug, Default)]
pub struct Login {
    #[garde(adapt(crate::forms::validate_url), length(max = 1000))]
    pub previous_uri: Option<Url>,
    #[garde(dive)]
    pub credentials: Credentials,
}

#[derive(Serialize, Deserialize, Validate, Debug, Default)]
pub struct Credentials {
    #[garde(pattern("^[a-zA-Z0-9_]+$"), length(min = 3, max = 50))]
    pub username: String,
    #[garde(length(min = 10, max = 100))]
    pub password: String,
}

#[derive(Deserialize)]
pub struct OidcLoginQuery {
    pub code: AuthorizationCode,
    pub state: CsrfToken,
}

#[derive(Serialize, Deserialize, Validate, Debug, Default)]
pub struct CreateOidcUser {
    #[garde(length(max = 500))]
    pub oidc_id: String,
    #[garde(length(max = 500))]
    pub email: String,
    #[garde(pattern("^[a-zA-Z0-9_]+$"), length(min = 3, max = 50))]
    pub username: String,
}
