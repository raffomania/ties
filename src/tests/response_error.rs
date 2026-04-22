use axum::{http::StatusCode, response::IntoResponse};

use crate::response_error::{self, ResponseError, ResponseResult};

#[tokio::test]
async fn not_authenticated_redirects_to_login() {
    let response = ResponseError::NotAuthenticated.into_response();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|h| h.to_str().ok()),
        Some("/login")
    );
}

#[tokio::test]
async fn not_found_returns_404() {
    let response = ResponseError::NotFound.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn anyhow_error_returns_500() {
    let err: ResponseError = anyhow::anyhow!("test error").into();
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn url_parse_error_returns_500() {
    let err = url::Url::parse("http://example.com:80:80").unwrap_err();
    let response = ResponseError::UrlParseError(err).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn into_option_does_not_convert_ok_to_none() {
    let ok_result: ResponseResult<()> = Ok(());
    assert!(matches!(
        response_error::into_option(ok_result),
        Ok(Some(()))
    ));
}

#[tokio::test]
async fn into_option_converts_not_found_to_none() {
    let not_found: ResponseResult<()> = Err(ResponseError::NotFound);
    assert!(matches!(response_error::into_option(not_found), Ok(None)));
}

#[tokio::test]
async fn into_option_does_not_convert_other_errs_to_none() {
    let test_cases = vec![
        ResponseError::NotAuthenticated,
        ResponseError::UrlParseError(url::Url::parse("http://example.com:80:80").unwrap_err()),
        ResponseError::FederationError(activitypub_federation::error::Error::Other(
            "test error".into(),
        )),
        ResponseError::Anyhow(anyhow::anyhow!("foo")),
        ResponseError::InvalidForm(crate::htmf_response::HtmfResponse(
            htmf::declare_inline::nothing(),
        )),
    ];

    for error in test_cases {
        let error_description = error.to_string();
        let result: ResponseResult<()> = Err(error);
        let actual = response_error::into_option(result);
        assert!(
            actual.is_err(),
            "Expected {error_description} error to be propagated, got {actual:?}"
        );
    }
}
