use crate::{ErrorPrint, MULTI_PROGRESS};

use bytes::Bytes;
use eyre::{eyre, Result};
use governor::{DefaultKeyedRateLimiter, Jitter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;
use ureq::http::StatusCode;
use ureq::{Agent, Body};
use url::Url;

pub fn get_text(url: &str) -> Result<String> {
    send_get_request_rec(url)?
        .read_to_string()
        .map_err(|e| eyre!("Broken link : {e} (URL: {url})"))
}

pub fn get_bytes(url: &str) -> Result<Bytes> {
    send_get_request_rec(url)?
        .with_config()
        .limit(100_000_000) // 100 MB
        .read_to_vec()
        .map(Into::<Bytes>::into)
        .map_err(|e| eyre!("Broken link : {e} (URL: {url})"))
}

fn send_get_request_rec(url: &str) -> Result<Body> {
    static BOUNCE: AtomicU8 = AtomicU8::new(0);
    #[allow(clippy::unwrap_used)]
    static RATE_LIMITER: LazyLock<DefaultKeyedRateLimiter<String>> = LazyLock::new(|| {
        RateLimiter::keyed(
            Quota::per_second(NonZeroU32::new(2u32).unwrap())
                .allow_burst(NonZeroU32::new(1u32).unwrap()),
        )
    });
    static AGENT: LazyLock<Agent> = LazyLock::new(|| {
        Agent::config_builder()
            .user_agent("AutEBook <https://github.com/ValentinLeTallec/AutEBook>")
            .build()
            .into()
    });

    let bounce = BOUNCE.load(Ordering::Relaxed);
    if bounce > 0 {
        let secs = 8 * 2_u64.pow(bounce.into());
        MULTI_PROGRESS.eprintln(&eyre!("Too many request, waiting for {secs} s"));
        thread::sleep(Duration::from_secs(secs));
    }

    let host = Url::parse(url)?
        .host()
        .map(|h| h.to_string())
        .unwrap_or_default();

    while RATE_LIMITER.check_key(&host).is_err() {
        thread::sleep(Jitter::up_to(Duration::from_millis(30)) + Duration::from_millis(50));
    }

    let response = AGENT
        .get(url)
        .call()
        .map_err(|e| eyre!("{e}, you might not be connected to the internet."))?;

    if response.status() == StatusCode::TOO_MANY_REQUESTS && bounce <= 10 {
        BOUNCE.fetch_add(1, Ordering::Relaxed);
        send_get_request_rec(url)
    } else {
        BOUNCE.swap(0, Ordering::Relaxed);
        Ok(response.into_body())
    }
}
