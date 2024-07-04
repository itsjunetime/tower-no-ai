#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

use std::{
	future::Future,
	pin::Pin,
	sync::OnceLock,
	task::{Context, Poll},
	time::{SystemTime, UNIX_EPOCH}
};

use http::{header::USER_AGENT, Request, Response, StatusCode};
use tower_layer::Layer;
use tower_service::Service;

/// The User-Agent patterns checked for and redirected if present
pub static AI_AGENTS: &[&str] = &[
	"AdsBot-Google2",
	"Amazonbot",
	"anthropic-ai",
	"Applebot",
	"ArcMobile",
	"AwarioRssBot",
	"AwarioSmartBot",
	"Bytespider",
	"CCBot",
	"ChatGPT-User",
	"Claude-Web",
	"ClaudeBot",
	"cohere-ai",
	"DataForSeoBot",
	"Diffbot",
	"FacebookBot",
	"FriendlyCrawler",
	"Google-Extended",
	"Googlebot-Image",
	"GoogleOther",
	"GPTBot",
	"ImagesiftBot",
	"img2dataset",
	"magpie-crawler",
	"Meltwater",
	"msnbot-media",
	"omgili",
	"omgilibot",
	"peer39_crawler",
	"PerplexityBot",
	"PiplBot",
	"Seekr",
	"scoop.it",
	"yandex",
	"YouBot"
];

/// The service which will redirect the requests with matching user agents
#[derive(Clone)]
pub struct NoAiService<S> {
	inner: S,
	layer: NoAiLayer
}

impl<S, ReqBody, RespBody> Service<Request<ReqBody>> for NoAiService<S>
where
	S: Service<Request<ReqBody>, Response = Response<RespBody>>,
	S::Future: Send + 'static,
	RespBody: Default
{
	type Error = S::Error;
	type Future =
		Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;
	type Response = Response<RespBody>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx)
	}

	fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
		// get the user agent
		req.headers()
			.get(USER_AGENT)
			// check if we can actually convert it to a string
			.and_then(|agent_hdr| agent_hdr.to_str().ok())
			// and then check that against all of the bad user agents we have stored
			.is_some_and(|agent| AI_AGENTS.iter().any(|hdr| agent.contains(hdr)))
			.then(|| -> Self::Future {
				// if it IS one of the bad user agents, then redirect it to our url and add the
				// extra query on the end to force refetching if we want that
				let redir_url = if self.layer.force_refetching {
					format!(
						"{}?={}",
						self.layer.redir_url,
						SystemTime::now()
							.duration_since(UNIX_EPOCH)
							.map_or(0, |d| d.as_nanos())
					)
				} else {
					self.layer.redir_url.clone()
				};

				Box::pin(async {
					Ok(Response::builder()
						.status(StatusCode::MOVED_PERMANENTLY)
						.header("Location", redir_url)
						.body(RespBody::default())
						.unwrap())
				})
			})
			// if it's not a bad user agent, let it continue
			.unwrap_or_else(move || -> Self::Future { Box::pin(self.inner.call(req)) })
	}
}

/// The [`tower`] layer which can be added to something like an [`axum::Router`]
///
/// [`tower`]: https://docs.rs/tower
/// [`axum::Router`]: https://docs.rs/axum/latest/axum/struct.Router.html
#[derive(Clone)]
pub struct NoAiLayer {
	redir_url: String,
	force_refetching: bool
}

impl NoAiLayer {
	/// Create a new `Self` which will redirect to the given URL when hit
	pub fn new(redir_url: impl Into<String>) -> Self {
		Self {
			redir_url: redir_url.into(),
			force_refetching: true
		}
	}

	/// Force any bots which are caught to re-fetch what ever address you give them by adding a new
	/// query (which query will change per-request) to the end of it.
	///
	/// If `force_refetching` is true, it will force the bot to re-fetch. This is the default. If
	/// `force_refetching` is false, it will not do so.
	pub fn force_refetching(mut self, force_refetching: bool) -> Self {
		self.force_refetching = force_refetching;
		self
	}
}

impl<S> Layer<S> for NoAiLayer {
	type Service = NoAiService<S>;
	fn layer(&self, inner: S) -> Self::Service {
		Self::Service {
			inner,
			layer: self.clone()
		}
	}
}

/// Returns the contents of a basic robots.txt file that explicitly disallows all the known AI bots
/// from accessing anything under the root of this website. Can be added with something like:
///
/// ```rust
/// use axum::routing::{get, Router};
/// use tower_no_ai::bot_blocking_robots_txt;
///
/// let router = Router::new()
///     .route("robots.txt", get(bot_blocking_robots_txt))
/// ```
pub fn bot_blocking_robots_txt() -> &'static str {
	static STORAGE: OnceLock<String> = OnceLock::new();

	STORAGE.get_or_init(|| {
		AI_AGENTS.iter().fold(String::new(), |txt, agent| {
			format!("{txt}User-Agent: {agent}\nDisallow: /\n")
		})
	})
}
