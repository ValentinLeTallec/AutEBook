use crate::{ErrorPrint, MULTI_PROGRESS};

use bytes::Bytes;
use eyre::{eyre, Result};
use governor::{DefaultKeyedRateLimiter, Jitter, Quota, RateLimiter};
use reqwest::blocking::{Client, Response};
use reqwest::StatusCode;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;
use url::Url;

pub fn get_text(url: &str) -> Result<String> {
    send_get_request_rec(url)?
        .error_for_status()
        .and_then(reqwest::blocking::Response::text)
        .map_err(|e| eyre!("Broken link : {e} (URL: {url})"))
}

pub fn get_bytes(url: &str) -> Result<Bytes> {
    send_get_request_rec(url)?
        .error_for_status()
        .and_then(reqwest::blocking::Response::bytes)
        .map_err(|e| eyre!("Broken link : {e} (URL: {url})"))
}

fn send_get_request_rec(url: &str) -> Result<Response> {
    static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);
    static BOUNCE: AtomicU8 = AtomicU8::new(0);
    #[allow(clippy::unwrap_used)]
    static RATE_LIMITER: LazyLock<DefaultKeyedRateLimiter<String>> = LazyLock::new(|| {
        RateLimiter::keyed(
            Quota::per_second(NonZeroU32::new(2u32).unwrap())
                .allow_burst(NonZeroU32::new(1u32).unwrap()),
        )
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

    let user_agent = "AutEBook <https://github.com/ValentinLeTallec/AutEBook>";
    let response = CLIENT.get(url).header("User-Agent", user_agent).send()?;

    if response.status() == StatusCode::TOO_MANY_REQUESTS && bounce <= 10 {
        BOUNCE.fetch_add(1, Ordering::Relaxed);
        send_get_request_rec(url)
    } else {
        BOUNCE.swap(0, Ordering::Relaxed);
        Ok(response)
    }
}
