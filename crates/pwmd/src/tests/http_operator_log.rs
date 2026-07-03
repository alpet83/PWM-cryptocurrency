//! Operator runtime log override HTTP tests.

use super::helpers::*;
use super::prelude::*;
use axum::extract::ConnectInfo;
use std::sync::Arc;

fn mk_log_app() -> App {
    let mut app = app_from_dev_net();
    app.log_ctl = Some(crate::logging::mk_test_log_ctl("info"));
    app
}

fn req_conn(req: Request<Body>, addr: SocketAddr) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(ConnectInfo(addr));
    Request::from_parts(parts, body)
}

#[tokio::test]
async fn op_log_set_get_del() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "debug",
        "focus": "api",
        "ttl_seconds": 30,
        "reason": "trace api issue"
    });
    let set_res = svc
        .clone()
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("set req"),
            SocketAddr::from(([127, 0, 0, 1], 5050)),
        ))
        .await
        .expect("set response");
    assert_eq!(set_res.status(), StatusCode::OK);
    let set_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(set_res.into_body(), 64 * 1024)
            .await
            .expect("set body"),
    )
    .expect("set json");
    assert_eq!(set_json["active"], true);
    assert_eq!(set_json["level"], "debug");
    assert_eq!(set_json["focus"], "api");

    let get_res = svc
        .clone()
        .oneshot(req_conn(
            Request::get("/v1/operator/log/override")
                .body(Body::empty())
                .expect("get req"),
            SocketAddr::from(([127, 0, 0, 1], 5050)),
        ))
        .await
        .expect("get response");
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_res.into_body(), 64 * 1024)
            .await
            .expect("get body"),
    )
    .expect("get json");
    assert_eq!(get_json["active"], true);
    assert_eq!(get_json["focus"], "api");

    let del_res = svc
        .clone()
        .oneshot(req_conn(
            Request::delete("/v1/operator/log/override")
                .body(Body::empty())
                .expect("del req"),
            SocketAddr::from(([127, 0, 0, 1], 5050)),
        ))
        .await
        .expect("del response");
    assert_eq!(del_res.status(), StatusCode::NO_CONTENT);

    let get_after = svc
        .oneshot(req_conn(
            Request::get("/v1/operator/log/override")
                .body(Body::empty())
                .expect("get2 req"),
            SocketAddr::from(([127, 0, 0, 1], 5050)),
        ))
        .await
        .expect("get2 response");
    let get_after_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_after.into_body(), 64 * 1024)
            .await
            .expect("get2 body"),
    )
    .expect("get2 json");
    assert_eq!(get_after_json["active"], false);
}

#[tokio::test]
async fn op_log_bad_focus() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "debug",
        "focus": "bad-focus",
        "ttl_seconds": 10
    });
    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("req"),
            SocketAddr::from(([127, 0, 0, 1], 5051)),
        ))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn op_log_bad_ttl() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "debug",
        "focus": "api",
        "ttl_seconds": 0
    });
    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("req"),
            SocketAddr::from(([127, 0, 0, 1], 5052)),
        ))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn op_log_remote_denied() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "debug",
        "focus": "api",
        "ttl_seconds": 10
    });
    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("req"),
            SocketAddr::from(([10, 10, 0, 9], 5053)),
        ))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_rpc_remote_denied() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();

    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/shutdown")
                .body(Body::empty())
                .expect("shutdown req"),
            SocketAddr::from(([10, 10, 0, 9], 5056)),
        ))
        .await
        .expect("shutdown response");

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn bridge_reset_remote_denied() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();

    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/bridge-federation/reset")
                .body(Body::empty())
                .expect("bridge reset req"),
            SocketAddr::from(([10, 10, 0, 9], 5057)),
        ))
        .await
        .expect("bridge reset response");

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn op_log_token_allows() {
    let mut app = mk_log_app();
    app.op_token = Some(Arc::<str>::from("pwmd-secret"));
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "info",
        "focus": "all",
        "ttl_seconds": 10
    });
    let res = svc
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .header("authorization", "Bearer pwmd-secret")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("req"),
            SocketAddr::from(([10, 10, 0, 9], 5054)),
        ))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn op_log_ttl_restore() {
    let app = mk_log_app();
    let svc = router_dev(app).into_service();
    let body = serde_json::json!({
        "level": "trace",
        "focus": "transport:peers",
        "ttl_seconds": 1
    });
    let set_res = svc
        .clone()
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("set req"),
            SocketAddr::from(([127, 0, 0, 1], 5055)),
        ))
        .await
        .expect("set response");
    assert_eq!(set_res.status(), StatusCode::OK);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    loop {
        let get_res = svc
            .clone()
            .oneshot(req_conn(
                Request::get("/v1/operator/log/override")
                    .body(Body::empty())
                    .expect("get req"),
                SocketAddr::from(([127, 0, 0, 1], 5055)),
            ))
            .await
            .expect("get response");
        assert_eq!(get_res.status(), StatusCode::OK);
        let get_json: serde_json::Value = serde_json::from_slice(
            &to_bytes(get_res.into_body(), 64 * 1024)
                .await
                .expect("get body"),
        )
        .expect("get json");
        if get_json["active"] == false {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "log override did not expire in time; last state={get_json}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn op_log_token_bad_bearer() {
    let mut app = mk_log_app();
    app.op_token = Some(Arc::<str>::from("pwmd-secret"));
    let svc = router_dev(app).into_service();
    let set_body = serde_json::json!({
        "level": "info",
        "focus": "api",
        "ttl_seconds": 30
    });
    let set_res = svc
        .clone()
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&set_body).expect("set json body"),
                ))
                .expect("set req"),
            SocketAddr::from(([127, 0, 0, 1], 5056)),
        ))
        .await
        .expect("set response");
    assert_eq!(set_res.status(), StatusCode::OK);

    let bad_body = serde_json::json!({
        "level": "trace",
        "focus": "all",
        "ttl_seconds": 10
    });
    let bad_res = svc
        .clone()
        .oneshot(req_conn(
            Request::post("/v1/operator/log/override")
                .header("content-type", "application/json")
                .header("authorization", "Bearer wrong-token")
                .body(Body::from(
                    serde_json::to_vec(&bad_body).expect("bad json body"),
                ))
                .expect("bad req"),
            SocketAddr::from(([10, 10, 0, 10], 5056)),
        ))
        .await
        .expect("bad response");
    assert_eq!(bad_res.status(), StatusCode::FORBIDDEN);

    let get_res = svc
        .oneshot(req_conn(
            Request::get("/v1/operator/log/override")
                .body(Body::empty())
                .expect("get req"),
            SocketAddr::from(([127, 0, 0, 1], 5056)),
        ))
        .await
        .expect("get response");
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_res.into_body(), 64 * 1024)
            .await
            .expect("get body"),
    )
    .expect("get json");
    assert_eq!(get_json["active"], true);
    assert_eq!(get_json["level"], "info");
    assert_eq!(get_json["focus"], "api");
}

#[tokio::test]
async fn op_log_tcp_smoke() {
    let mut app = mk_log_app();
    app.op_token = Some(Arc::<str>::from("pwmd-secret"));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router = router_dev(app);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut stream = loop {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(stream) => break stream,
            Err(err) => {
                if tokio::time::Instant::now() >= deadline {
                    panic!("connect timeout: {err}");
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
    };
    let raw_req = concat!(
        "GET /v1/operator/log/override HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Authorization: Bearer pwmd-secret\r\n",
        "Connection: close\r\n\r\n"
    );
    stream
        .write_all(raw_req.as_bytes())
        .await
        .expect("write req");
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.expect("read resp");
    server.abort();
    let text = String::from_utf8_lossy(&resp);
    assert!(
        text.starts_with("HTTP/1.1 200"),
        "expected 200 status line, got: {text}"
    );
    assert!(
        text.contains("\"active\":false"),
        "expected inactive override in body, got: {text}"
    );
}
