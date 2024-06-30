# `tower-no-ai`

A very simple crate to extend tower with the ability to redirect all AI-scraper bot User-Agent headers to a user-defined URL.

This can be done with something like the following (with axum):

```rust
use tower_no_ai::NoAiLayer;
use axum::routing::{get, Router};
let route = Router::new()
	.route("/", get(hello_world))
	// route them to a hetzner 10GB speed test file
	.layer(NoAiLayer::new("https://fsn1-speed.hetzner.com/10GB.bin"));
```

As this is built on `tower`, it should work perfectly with all tower-based backends.

Contributions, bug reports, and suggestions are welcome.

Dual-Licensed MIT and Apache 2.0
