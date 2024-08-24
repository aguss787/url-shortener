use std::sync::Arc;

use authenthication::{AuthenticationService, Requester};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use config::Config;
use http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, Method, StatusCode,
};
use kvs::kvs_pool;
use requests::{AuthRequest, ListUrl, NewUrl, RedirectUrlIdPathParam, RedirectUrlPathParam};
use responses::{AuthResponse, MeResponse, PagedResponse, UrlRedirect};
use service::{NewUrlRedirect, UrlService};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing_subscriber::EnvFilter;

// Auto generated by sea-orm
#[allow(unused_imports)]
mod models;

mod authenthication;
mod config;
mod kvs;
mod requests;
mod responses;
mod service;

struct Services {
    pub url: UrlService,
    pub auth: AuthenticationService,
}

impl Services {
    fn new(url: UrlService, auth: AuthenticationService) -> Self {
        Self { url, auth }
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv::from_filename(".env").ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .with_file(true)
        .with_line_number(true)
        .init();

    let config = Config::read_env();
    let port = config.port;

    let kvs_pool =
        Arc::new(kvs_pool(&config.kvs_url).expect("Failed to create KVS connection pool"));

    let services = Services::new(
        UrlService::new(&config.postgres_url).await,
        AuthenticationService::new(
            config.agus_dev_sso_host,
            config.client_id,
            config.client_secret,
            config.redirect_uri,
            kvs_pool,
        ),
    );

    let cors = CorsLayer::new()
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_origin(AllowOrigin::list(
            config
                .allowed_origins
                .iter()
                .map(|origin| HeaderValue::from_str(origin).expect("invalid origin")),
        ))
        .allow_headers(vec![AUTHORIZATION, CONTENT_TYPE])
        .allow_credentials(true);

    let app = Router::new()
        .route("/auth/callback", post(auth_callback))
        .route("/me", get(me_handler))
        .route("/urls/redirect/:key", get(redirect_handler))
        .route("/urls", get(get_urls).post(new_url))
        .route(
            "/urls/:id",
            get(get_url).delete(delete_url).patch(update_url),
        )
        .with_state(Arc::new(services))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .layer(cors);

    tracing::info!("Listening on 0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

async fn redirect_handler(
    Path(RedirectUrlPathParam { key }): Path<RedirectUrlPathParam>,
    service: State<Arc<Services>>,
) -> Result<Response, Response> {
    let result = service.url.get_by_key(&key).await?;

    match result {
        None => Ok((StatusCode::NOT_FOUND, "not found").into_response()),
        Some(redirect) => Ok(axum::response::Redirect::permanent(&redirect.target).into_response()),
    }
}

async fn new_url(
    requester: Requester,
    service: State<Arc<Services>>,
    Json(new_url): Json<NewUrl>,
) -> Result<Json<UrlRedirect>, Response> {
    service
        .url
        .insert(NewUrlRedirect::new(
            requester.email,
            new_url.key,
            new_url.target,
        ))
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn delete_url(
    requester: Requester,
    service: State<Arc<Services>>,
    Path(RedirectUrlIdPathParam { id }): Path<RedirectUrlIdPathParam>,
) -> Result<Json<UrlRedirect>, Response> {
    service
        .url
        .delete(&requester.email, id)
        .await
        .map_err(Into::into)
        .and_then(|o| o.ok_or_else(|| (StatusCode::NOT_FOUND, "not found").into_response()))
        .map(Json)
}

async fn update_url(
    requester: Requester,
    service: State<Arc<Services>>,
    Path(RedirectUrlIdPathParam { id }): Path<RedirectUrlIdPathParam>,
    Json(new_url): Json<NewUrl>,
) -> Result<Json<UrlRedirect>, Response> {
    service
        .url
        .update(
            id,
            NewUrlRedirect::new(requester.email, new_url.key, new_url.target),
        )
        .await
        .map_err(Into::into)
        .and_then(|o| o.ok_or_else(|| (StatusCode::NOT_FOUND, "not found").into_response()))
        .map(Json)
}

async fn get_urls(
    requester: Requester,
    service: State<Arc<Services>>,
    Query(query): Query<ListUrl>,
) -> Result<Json<PagedResponse<UrlRedirect>>, Response> {
    let result = service
        .url
        .list_by_email(&requester.email, query.after, query.limit.unwrap_or(50))
        .await?;

    Ok(Json(PagedResponse::new(result)))
}

async fn get_url(
    requester: Requester,
    service: State<Arc<Services>>,
    Path(RedirectUrlIdPathParam { id }): Path<RedirectUrlIdPathParam>,
) -> Result<Json<UrlRedirect>, Response> {
    service
        .url
        .get_by_id_and_email(id, &requester.email)
        .await
        .map_err(Into::into)
        .and_then(|o| o.ok_or_else(|| (StatusCode::NOT_FOUND, "not found").into_response()))
        .map(Json)
}

async fn auth_callback(
    service: State<Arc<Services>>,
    Json(AuthRequest { authorization_code }): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, Response> {
    let access_token = service.auth.exchange_token(&authorization_code).await?;

    Ok(Json(access_token))
}

async fn me_handler(requester: Requester) -> Result<Json<MeResponse>, Response> {
    Ok(Json(MeResponse::new(requester.email)))
}
