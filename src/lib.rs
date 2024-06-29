use std::{future::Future, pin::Pin, task::{Context, Poll}};

use tower_layer::Layer;
use tower_service::Service;
use http::{header::USER_AGENT, Request, Response, StatusCode};

static AI_AGENTS: [&str; 34] = [
	"AdsBot-Google2",
	"Amazonbot",
	"anthropic-ai",
	"Applebot",
	"ArcMobile",
	"AwarioRssBot",
	"Bytespider",
	"CCBot",
	"ChatGPT-User",
	"Claude-Web",
	"ClaudeBot",
	"cohere-ai",
	"DataForSeoBot",
	"DiffBot",
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

#[derive(Clone)]
pub struct NoAiService<S> {
	inner: S,
	redir_url: String,
}

impl<S, ReqBody, RespBody> Service<Request<ReqBody>> for NoAiService<S>
where
	S: Service<Request<ReqBody>, Response = Response<RespBody>>,
	S::Future: Send + 'static,
	RespBody: Default,
{
	type Error = S::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;
	type Response = Response<RespBody>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx)
	}

	fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
		// get the user agent
		req.headers().get(USER_AGENT)
			// check if we can actually convert it to a string
			.and_then(|agent_hdr| agent_hdr.to_str().ok())
			// and then check that against all of the bad user agents we have stored
			.is_some_and(|agent| AI_AGENTS.iter().any(|hdr| agent.contains(hdr)))
			.then(|| -> Self::Future {
				// if it IS one of the bad user agents, then redirect it to our url
				let redir_url = self.redir_url.clone();
				Box::pin(async {
					Ok(Response::builder()
						.status(StatusCode::MOVED_PERMANENTLY)
						.header("Location", redir_url)
						.body(RespBody::default())
						.unwrap())
				})
			})
			// if it's not a bad user agent, let it continue
			.unwrap_or_else(move || -> Self::Future {
				Box::pin(self.inner.call(req))
			})
	}
}

#[derive(Clone)]
pub struct NoAiLayer {
	redir_url: String
}

impl NoAiLayer {
	pub fn new(redir_url: impl Into<String>) -> Self {
		Self { redir_url: redir_url.into() }
	}
}

impl<S> Layer<S> for NoAiLayer {
	type Service = NoAiService<S>;
	fn layer(&self, inner: S) -> Self::Service {
		Self::Service {
			inner,
			redir_url: self.redir_url.clone()
		}
	}
}
