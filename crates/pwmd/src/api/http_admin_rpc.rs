#[cfg(test)]
mod http_admin_rpc_tests {
    use crate::api::handlers_bridge::v1_bridge_federation_reset;
    use crate::api::handlers_shutdown::v1_shutdown;
    use crate::bootstrap::app_from_dev_net;
    use axum::extract::{ConnectInfo, State};
    use axum::http::{header::AUTHORIZATION, HeaderMap, HeaderValue, StatusCode};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;

    #[tokio::test]
    async fn shutdown_remote_no_token_forbidden() {
        let app = app_from_dev_net();
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 1234)));
        let headers = HeaderMap::new();
        let res = v1_shutdown(State(app.clone()), conn, headers).await;
        assert!(matches!(res, Err((StatusCode::FORBIDDEN, _))));
    }

    #[tokio::test]
    async fn shutdown_loopback_no_token_passes() {
        let app = app_from_dev_net();
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)));
        let headers = HeaderMap::new();
        let res = v1_shutdown(State(app.clone()), conn, headers).await;
        assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn shutdown_token_nonloopback_passes() {
        let mut app = app_from_dev_net();
        app.op_token = Some(Arc::from("secret"));
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)), 4444)));
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        let res = v1_shutdown(State(app.clone()), conn, headers).await;
        assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn bridge_reset_remote_no_token_forbidden() {
        let app = app_from_dev_net();
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 1234)));
        let headers = HeaderMap::new();
        let res = v1_bridge_federation_reset(State(app.clone()), conn, headers).await;
        assert!(matches!(res, Err((StatusCode::FORBIDDEN, _))));
    }

    #[tokio::test]
    async fn bridge_reset_loopback_no_token_passes() {
        let app = app_from_dev_net();
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)));
        let headers = HeaderMap::new();
        let res = v1_bridge_federation_reset(State(app.clone()), conn, headers).await;
        assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn bridge_reset_token_nonloopback_passes() {
        let mut app = app_from_dev_net();
        app.op_token = Some(Arc::from("sometoken"));
        let conn = Some(ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 4444)));
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer sometoken"));
        let res = v1_bridge_federation_reset(State(app.clone()), conn, headers).await;
        assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
    }
}
