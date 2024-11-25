use async_trait::async_trait;
use bytes::Bytes;
use pingora::prelude::Opt;
use pingora::server::configuration::ServerConf;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::Result;
use pingora_http::ResponseHeader;
use pingora_load_balancing::Backend;
use pingora_proxy::ProxyHttp;
use pingora_proxy::Session;
use sd_notify::NotifyState;
use std::borrow::Cow;
use std::sync::Arc;

// https://github.com/cloudflare/pingora/blob/e309436319ed5cbc3aaf53221070a1fd070b8bcf/docs/user_guide/graceful.md?plain=1#L9
fn main() {
    let parent_pid = std::env::args().nth(1);
    println!("parent_pid: {:?}", std::env::args());
    let mut upgrade = false;
    if parent_pid.is_some() {
        upgrade = true;
    }

    env_logger::init();
    let mut server = Server::new(Some(Opt {
        upgrade,
        daemon: false,
        nocapture: false,
        test: false,
        conf: None,
    }))
    .unwrap();
    server.configuration = Arc::new(ServerConf {
        // What should this be? Maybe number of processes? But maybe we want to
        // limit how many cores this LB can occupy and leave the rest for Deno
        // workers.
        threads: 8,

        pid_file: String::from("/tmp/pingora.pid"),
        upgrade_sock: String::from("/tmp/pingora_upgrade.sock"),

        // These are all default values.
        version: 1,
        error_log: None,
        daemon: false,
        user: None,
        group: None,
        work_stealing: true,
        ca_file: None,
        grace_period_seconds: Some(10),
        graceful_shutdown_timeout_seconds: Some(10),
        client_bind_to_ipv4: vec![],
        client_bind_to_ipv6: vec![],
        upstream_keepalive_pool_size: 128,
        upstream_connect_offload_threadpools: None,
        upstream_connect_offload_thread_per_pool: None,
        upstream_debug_ssl_keylog: false,
    });
    server.bootstrap();
    sd_notify::notify(false, &[NotifyState::Ready]).unwrap();

    let mut lb = pingora_proxy::http_proxy_service(
        &server.configuration,
        LB {
            uuid: uuid::Uuid::new_v4(),
        },
    );
    lb.add_tcp("0.0.0.0:6188");

    server.add_service(lb);

    server.run_forever();
}

struct LB {
    uuid: uuid::Uuid,
}

impl LB {
    async fn send_response<'a, T>(
        &self,
        session: &mut Session,
        status: u16,
        headers: Vec<(&'static str, &'static str)>,
        body: T,
    ) -> Result<bool>
    where
        T: Into<Cow<'a, str>>,
    {
        let mut header = ResponseHeader::build(status, None).unwrap();
        for (key, value) in headers {
            header.insert_header(key, value).unwrap();
        }
        session
            .write_response_header(Box::new(header), true)
            .await?;

        let body_str: Cow<'a, str> = body.into();
        let body_bytes = Bytes::from(body_str.into_owned());

        session.write_response_body(Some(body_bytes), true).await?;
        session.set_keepalive(None);
        Ok(true)
    }
    async fn send_hello_world_response(&self, session: &mut Session) -> Result<bool> {
        self.send_response(
            session,
            200,
            vec![("content-type", "text/plain")],
            format!("Hello, world! (instance: {})", self.uuid),
        )
        .await
    }
}

struct Ctx {}

#[async_trait]
impl ProxyHttp for LB {
    /// For this small example, we don't need context storage
    type CTX = Ctx;
    fn new_ctx(&self) -> Self::CTX {
        Ctx {}
    }

    /// Handle the incoming request.
    ///
    /// In this phase, users can parse, validate, rate limit, perform access control and/or
    /// return a response for this request.
    ///
    /// If the user already sent a response to this request, an `Ok(true)` should be returned so that
    /// the proxy would exit. The proxy continues to the next phases when `Ok(false)` is returned.
    ///
    /// By default this filter does nothing and returns `Ok(false)`.
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        self.send_hello_world_response(session).await
    }

    /// Define where the proxy should send the request to.
    ///
    /// The returned [HttpPeer] contains the information regarding where and how this request should
    /// be forwarded to.
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        Ok(Box::new(HttpPeer::new(
            // Junk, shouldn't get here.
            Backend::new("127.0.0.1:80").unwrap(),
            false,
            "".to_string(),
        )))
    }
}
