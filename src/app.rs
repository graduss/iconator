use std::path::{Component, Path, PathBuf};

use iconator::{get_icon_for_file, get_icon_for_folder};

use axum::{
    Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

pub fn get_server() -> Router<AppState> {
    Router::new().route("/api/v1/lookup", get(lookup))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[default]
    File,
    Folder,
}

#[derive(Debug, Deserialize)]
struct LookupQuery {
    path: String,
    #[serde(default)]
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppError {
    IncorrectPath,
    NotFound,
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::IncorrectPath => (StatusCode::BAD_REQUEST, "Incorrect path"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "File not found"),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        }
        .into_response()
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub icons_dir: PathBuf,
}

async fn lookup(
    Query(q): Query<LookupQuery>,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    let path = Path::new(&q.path);

    if !matches!(path.components().next_back(), Some(Component::Normal(_))) {
        return Err(AppError::IncorrectPath);
    }

    let icon_id = match q.kind {
        Kind::File => get_icon_for_file(path).ok_or(AppError::NotFound),
        Kind::Folder => get_icon_for_folder(path).ok_or(AppError::NotFound),
    }?;

    let svg = get_svg(icon_id, &state.icons_dir).await?;

    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

async fn get_svg(icon_id: u64, icons_dir: &Path) -> Result<Vec<u8>, AppError> {
    let file = icons_dir.join(format!("{icon_id}.svg"));

    tokio::fs::read(&file).await.map_err(|err| {
        tracing::error!("failed to read {}: {err}", file.display());
        AppError::Internal
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    const ICONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/icons");

    fn app(icons_dir: &str) -> Router {
        get_server().with_state(AppState {
            icons_dir: PathBuf::from(icons_dir),
        })
    }

    async fn get(app: Router, uri: &str) -> Response {
        app.oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn file_returns_svg() {
        let res = get(app(ICONS_DIR), "/api/v1/lookup?path=src/main.rs").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "image/svg+xml");

        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let expected = std::fs::read(format!("{ICONS_DIR}/525.svg")).unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn folder_returns_svg() {
        let res = get(app(ICONS_DIR), "/api/v1/lookup?path=.github&kind=folder").await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "image/svg+xml");
    }

    #[tokio::test]
    async fn error_statuses() {
        let cases = [
            ("/api/v1/lookup?path=unknown_file", StatusCode::NOT_FOUND),
            ("/api/v1/lookup?path=src&kind=file", StatusCode::NOT_FOUND),
            ("/api/v1/lookup?path=..", StatusCode::BAD_REQUEST),
            ("/api/v1/lookup?path=", StatusCode::BAD_REQUEST),
            ("/api/v1/lookup?path=a.rs&kind=bad", StatusCode::BAD_REQUEST),
            ("/api/v1/lookup", StatusCode::BAD_REQUEST),
        ];

        for (uri, expected) in cases {
            assert_eq!(get(app(ICONS_DIR), uri).await.status(), expected, "{uri}");
        }
    }

    #[tokio::test]
    async fn missing_icons_dir_is_internal_error() {
        let res = get(app("/nonexistent"), "/api/v1/lookup?path=main.rs").await;

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
