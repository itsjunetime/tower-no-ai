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
	"AI2Bot",
	"Ai2Bot-Dolma",
	"AdsBot-Google2",
	"Amazonbot",
	"anthropic-ai",
	"Applebot",
	"Applebot-Extended",
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
	"DuckAssistBot",
	"FacebookBot",
	"FriendlyCrawler",
	"Google-Extended",
	"Googlebot-Image",
	"GoogleOther",
	"GoogleOther-Image",
	"GoogleOther-Video",
	"GPTBot",
	"iaskspider/2.0",
	"ICC-Crawler",
	"ImagesiftBot",
	"img2dataset",
	"ISSCyberRiskCrawler",
	"Kangaroo Bot",
	"Meta-ExternalAgent",
	"Meta-ExternalFetcher",
	"OAI-SearchBot",
	"magpie-crawler",
	"Meltwater",
	"msnbot-media",
	"omgili",
	"omgilibot",
	"PanguBot",
	"peer39_crawler",
	"PerplexityBot",
	"PetalBot",
	"PiplBot",
	"Scrapy",
	"Seekr",
	"Sidetrade indexer bot",
	"scoop.it",
	"Timpibot",
	"VelenPublicWebCrawler",
	"Webzio-Extended",
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
	type Future = ServiceFut<RespBody, Self::Error, S::Future>;
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

				ServiceFut::Redirect(redir_url)
			})
			// if it's not a bad user agent, let it continue
			.unwrap_or_else(move || ServiceFut::Inner(self.inner.call(req)))
	}
}

/// The Future type that [`NoAiService::call`] produces. This has the bounds necessary to work
/// nicely with the [`tower_service::Service`] API requirements for the associated `Future` type.
pub enum ServiceFut<RespBody, Err, F>
where
	RespBody: Default,
	F: Future<Output = Result<Response<RespBody>, Err>>
{
	/// This variant is created when the [`NoAiService`] doesn't find an AI USER_AGENT header in an
	/// incoming request, and so just forwards the request on to the next service in the stack. The
	/// `F` type is just the future that that next service returns.
	Inner(F),
	/// This variant is created with the [`NoAiService`] DOES find an AI USER_AGENT header and thus
	/// redirects the request. The wrapped [`String`] is the url that it will be redirected to.
	Redirect(String)
}

impl<RespBody, Err, F> Future for ServiceFut<RespBody, Err, F>
where
	RespBody: Default,
	F: Future<Output = Result<Response<RespBody>, Err>>
{
	type Output = Result<Response<RespBody>, Err>;
	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		// SAFETY: This is safe because we guarantee that we don't move out of the mutable
		// reference this produces. We just need to match on &mut values here so that we can poll
		// the inner future.
		match unsafe { self.get_unchecked_mut() } {
			Self::Redirect(redir_url) => Poll::Ready(Ok(Response::builder()
				.status(StatusCode::MOVED_PERMANENTLY)
				.header("Location", &*redir_url)
				.body(RespBody::default())
				.unwrap())),
			// SAFETY: This is safe because we matched on a reference, so it hasn't moved since we
			// looked at it inside the `Pin` over `&mut Self` above.
			Self::Inner(f) => unsafe { Pin::new_unchecked(f) }.poll(cx)
		}
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
	#[must_use]
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
