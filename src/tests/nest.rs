use super::*;
use crate::body::box_body;
use std::collections::HashMap;

#[tokio::test]
async fn nesting_apps() {
    let api_routes = Router::new()
        .route(
            "/users",
            get(|| async { "users#index" }).post(|| async { "users#create" }),
        )
        .route(
            "/users/:id",
            get(
                |params: extract::Path<HashMap<String, String>>| async move {
                    dbg!(&params);
                    format!(
                        "{}: users#show ({})",
                        params.get("version").unwrap(),
                        params.get("id").unwrap()
                    )
                },
            ),
        )
        .route(
            "/games/:id",
            get(
                |params: extract::Path<HashMap<String, String>>| async move {
                    format!(
                        "{}: games#show ({})",
                        params.get("version").unwrap(),
                        params.get("id").unwrap()
                    )
                },
            ),
        );

    let app = Router::new()
        .route("/", get(|| async { "hi" }))
        .nest("/:version/api", api_routes);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "hi");

    let res = client.get("/v0/api/users").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#index");

    let res = client.get("/v0/api/users/123").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "v0: users#show (123)");

    let res = client.get("/v0/api/games/123").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "v0: games#show (123)");
}

#[tokio::test]
async fn wrong_method_nest() {
    let nested_app = Router::new().route("/", get(|| async {}));
    let app = Router::new().nest("/", nested_app);

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/").send().await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);

    let res = client.patch("/foo").send().await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nesting_at_root() {
    let app = Router::new().nest("/", get(|uri: Uri| async move { uri.to_string() }));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/");

    let res = client.get("/foo").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo");

    let res = client.get("/foo/bar").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo/bar");
}

#[tokio::test]
async fn nested_url_extractor() {
    let app = Router::new().nest(
        "/foo",
        Router::new().nest(
            "/bar",
            Router::new()
                .route("/baz", get(|uri: Uri| async move { uri.to_string() }))
                .route(
                    "/qux",
                    get(|req: Request<Body>| async move { req.uri().to_string() }),
                ),
        ),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/baz");

    let res = client.get("/foo/bar/qux").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/qux");
}

#[tokio::test]
async fn nested_url_original_extractor() {
    let app = Router::new().nest(
        "/foo",
        Router::new().nest(
            "/bar",
            Router::new().route(
                "/baz",
                get(|uri: extract::OriginalUri| async move { uri.0.to_string() }),
            ),
        ),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/foo/bar/baz");
}

#[tokio::test]
async fn nested_service_sees_stripped_uri() {
    let app = Router::new().nest(
        "/foo",
        Router::new().nest(
            "/bar",
            Router::new().route(
                "/baz",
                service_fn(|req: Request<Body>| async move {
                    let body = box_body(Body::from(req.uri().to_string()));
                    Ok::<_, Infallible>(Response::new(body))
                }),
            ),
        ),
    );

    let client = TestClient::new(app);

    let res = client.get("/foo/bar/baz").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "/baz");
}

#[tokio::test]
async fn nest_static_file_server() {
    let app = Router::new().nest(
        "/static",
        service::get(tower_http::services::ServeDir::new(".")).handle_error(|error| {
            Ok::<_, Infallible>((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled internal error: {}", error),
            ))
        }),
    );

    let client = TestClient::new(app);

    let res = client.get("/static/README.md").send().await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn nested_multiple_routes() {
    let app = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/users", get(|| async { "users" }))
                .route("/teams", get(|| async { "teams" })),
        )
        .route("/", get(|| async { "root" }));

    let client = TestClient::new(app);

    assert_eq!(client.get("/").send().await.text().await, "root");
    assert_eq!(client.get("/api/users").send().await.text().await, "users");
    assert_eq!(client.get("/api/teams").send().await.text().await, "teams");
}

#[tokio::test]
async fn nested_with_other_route_also_matching_with_route_first() {
    let app = Router::new().route("/api", get(|| async { "api" })).nest(
        "/api",
        Router::new()
            .route("/users", get(|| async { "users" }))
            .route("/teams", get(|| async { "teams" })),
    );

    let client = TestClient::new(app);

    assert_eq!(client.get("/api").send().await.text().await, "api");
    assert_eq!(client.get("/api/users").send().await.text().await, "users");
    assert_eq!(client.get("/api/teams").send().await.text().await, "teams");
}

#[tokio::test]
async fn nested_with_other_route_also_matching_with_route_last() {
    let app = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/users", get(|| async { "users" }))
                .route("/teams", get(|| async { "teams" })),
        )
        .route("/api", get(|| async { "api" }));

    let client = TestClient::new(app);

    assert_eq!(client.get("/api").send().await.text().await, "api");
    assert_eq!(client.get("/api/users").send().await.text().await, "users");
    assert_eq!(client.get("/api/teams").send().await.text().await, "teams");
}

#[tokio::test]
async fn multiple_top_level_nests() {
    let app = Router::new()
        .nest(
            "/one",
            Router::new().route("/route", get(|| async { "one" })),
        )
        .nest(
            "/two",
            Router::new().route("/route", get(|| async { "two" })),
        );

    let client = TestClient::new(app);

    assert_eq!(client.get("/one/route").send().await.text().await, "one");
    assert_eq!(client.get("/two/route").send().await.text().await, "two");
}
