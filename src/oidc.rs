use anyhow::{Context as _, anyhow};
use openidconnect::{
    AccessTokenHash, AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse as _, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    Scope,
    core::{CoreClient, CoreIdTokenVerifier, CoreProviderMetadata, CoreResponseType},
    reqwest,
    url::Url,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{cli::OidcArgs, response_error::ResponseResult};

#[derive(Serialize, Deserialize)]
pub struct AuthenticatedOidcUserInfo {
    pub oidc_id: String,
    pub email: String,
}

impl AuthenticatedOidcUserInfo {
    const SESSION_KEY: &'static str = "oidc_user_info";

    pub async fn save_in_session(self, session: &Session) -> ResponseResult<()> {
        session
            .insert(Self::SESSION_KEY, self)
            .await
            .context("Failed to insert oidc data into session")?;

        Ok(())
    }

    pub async fn from_session(session: &Session) -> ResponseResult<Self> {
        Ok(session
            .get(Self::SESSION_KEY)
            .await
            .context("Failed to load oidc data from session")?
            .context("oidc data not found in session")?)
    }
}

#[derive(Serialize, Deserialize)]
pub struct LoginAttempt {
    pub nonce: Nonce,
    pub csrf_token: CsrfToken,
    pub pkce_verifier: PkceCodeVerifier,
    pub authorize_url: Url,
}

impl LoginAttempt {
    const SESSION_KEY: &'static str = "oidc_login_attempt";

    pub fn new(client: &ConfiguredClient) -> Self {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate the authorization URL to which we'll redirect the user.
        let (authorize_url, csrf_token, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        LoginAttempt {
            nonce,
            csrf_token,
            pkce_verifier,
            authorize_url,
        }
    }

    pub async fn save_in_session(self, session: &Session) -> ResponseResult<()> {
        session
            .insert(Self::SESSION_KEY, self)
            .await
            .context("Failed to insert login attempt into session")?;

        Ok(())
    }

    pub async fn from_session(session: &Session) -> ResponseResult<Self> {
        Ok(session
            .get(Self::SESSION_KEY)
            .await
            .context("Failed to load oidc login attempt from session")?
            .context("oidc login attempt not found in session")?)
    }

    pub async fn login(
        self,
        oidc_client: &ConfiguredClient,
        reqwest_client: &reqwest::Client,
        csrf_token: CsrfToken,
        code: AuthorizationCode,
    ) -> ResponseResult<AuthenticatedOidcUserInfo> {
        if csrf_token.secret() != self.csrf_token.secret() {
            return Err(anyhow!("CSRF token mismatch").into());
        }
        let token_response = oidc_client
            .clone()
            .exchange_code(code)
            .context("Failed to set exchange code")?
            .set_pkce_verifier(self.pkce_verifier)
            .request_async(reqwest_client)
            .await
            .context("failed to get token response")?;
        let id_token_verifier: CoreIdTokenVerifier = oidc_client.id_token_verifier();
        let id_token = token_response
            .extra_fields()
            .id_token()
            .context("Server did not return an ID token")?;
        let id_token_claims = id_token
            .claims(&id_token_verifier, &self.nonce)
            .context("failed to get token claims")?;

        if let Some(expected_access_token_hash) = id_token_claims.access_token_hash() {
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                id_token
                    .signing_alg()
                    .context("failed to get signing algorithm")?,
                id_token
                    .signing_key(&id_token_verifier)
                    .context("Failed to get signing key")?,
            )
            .context("Failed to get access token hash from token response")?;
            if actual_access_token_hash != *expected_access_token_hash {
                return Err(anyhow!("Invalid access token").into());
            }
        }

        let email = id_token_claims
            .email()
            .context("failed to get email")?
            .to_string();

        let oidc_id = id_token_claims.subject().to_string();

        Ok(AuthenticatedOidcUserInfo { oidc_id, email })
    }
}

type ConfiguredClient = openidconnect::Client<
    openidconnect::EmptyAdditionalClaims,
    openidconnect::core::CoreAuthDisplay,
    openidconnect::core::CoreGenderClaim,
    openidconnect::core::CoreJweContentEncryptionAlgorithm,
    openidconnect::core::CoreJsonWebKey,
    openidconnect::core::CoreAuthPrompt,
    openidconnect::StandardErrorResponse<openidconnect::core::CoreErrorResponseType>,
    openidconnect::StandardTokenResponse<
        openidconnect::IdTokenFields<
            openidconnect::EmptyAdditionalClaims,
            openidconnect::EmptyExtraTokenFields,
            openidconnect::core::CoreGenderClaim,
            openidconnect::core::CoreJweContentEncryptionAlgorithm,
            openidconnect::core::CoreJwsSigningAlgorithm,
        >,
        openidconnect::core::CoreTokenType,
    >,
    openidconnect::StandardTokenIntrospectionResponse<
        openidconnect::EmptyExtraTokenFields,
        openidconnect::core::CoreTokenType,
    >,
    openidconnect::core::CoreRevocableToken,
    openidconnect::StandardErrorResponse<openidconnect::RevocationErrorResponseType>,
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

#[derive(Clone)]
pub struct Config {
    pub client: ConfiguredClient,
    pub reqwest_client: reqwest::Client,
    pub name: String,
}

#[derive(Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "It's expected since it works like an Option."
)]
pub enum State {
    NotConfigured,
    Configured(Config),
}

impl State {
    #[must_use]
    pub fn get_config(self) -> Option<Config> {
        match self {
            State::NotConfigured => None,
            State::Configured(config) => Some(config),
        }
    }

    pub async fn initialize(base_url: &Url, args: Option<OidcArgs>) -> State {
        match Self::try_initialize_state(base_url, args).await {
            Ok(conf) => {
                tracing::info!("OIDC enabled.");
                State::Configured(conf)
            }
            Err(err) => {
                tracing::info!("OIDC disabled: {err:?}");
                State::NotConfigured
            }
        }
    }

    async fn try_initialize_state(
        base_url: &Url,
        args: Option<OidcArgs>,
    ) -> anyhow::Result<Config> {
        let reqwest_client = openidconnect::reqwest::ClientBuilder::new()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .build()
            .context("Failed to build reqwest client")?;

        let args = args.context("OIDC configuration is absent or incomplete.")?;
        let client_id = ClientId::new(args.oidc_client_id);
        let client_secret = ClientSecret::new(args.oidc_client_secret.expose_secret().clone());
        let issuer_url =
            IssuerUrl::new(args.oidc_issuer_url).context("failed to parse issuer URL")?;

        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &reqwest_client)
            .await
            .context("failed to discover provider")?;
        // Set up the config for the OIDC process.
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(RedirectUrl::from_url(
                    base_url.join("/login_oidc_redirect")?,
                ));

        Ok(Config {
            client,
            reqwest_client,
            name: args.oidc_issuer_name,
        })
    }
}
