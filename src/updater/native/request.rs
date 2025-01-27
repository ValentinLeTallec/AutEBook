use crate::{ErrorPrint, MULTI_PROGRESS};

use eyre::{eyre, Result};
use governor::{DefaultKeyedRateLimiter, Jitter, Quota, RateLimiter};
use reqwest::blocking::{Client, Response};
use reqwest::StatusCode;
use std::num::NonZeroU32;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;
use url::Url;

const USER_AGENT: &str = "AutEBook <https://github.com/ValentinLeTallec/AutEBook>";

pub fn get(url: &str) -> Result<Response> {
    send_get_request_rec(url, 4)
}

fn send_get_request_rec(url: &str, bounce: u8) -> Result<Response> {
    static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);
    #[allow(clippy::unwrap_used)]
    static RATE_LIMITER: LazyLock<DefaultKeyedRateLimiter<String>> = LazyLock::new(|| {
        RateLimiter::keyed(
            Quota::per_second(NonZeroU32::new(2u32).unwrap())
                .allow_burst(NonZeroU32::new(1u32).unwrap()),
        )
    });

    let host = Url::parse(url)
        .map_err(|e| eyre!("{e} (URL: {url})"))?
        .host()
        .map(|h| h.to_string())
        .unwrap_or_default();

    while RATE_LIMITER.check_key(&host).is_err() {
        thread::sleep(Jitter::up_to(Duration::from_millis(30)) + Duration::from_millis(50));
    }

    CLIENT
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| eyre!("{e} (URL: {url})"))
        .and_then(|e| {
            if e.status() == StatusCode::TOO_MANY_REQUESTS && bounce <= 10 {
                let secs = 2_u8.pow(bounce.into()).into();
                MULTI_PROGRESS.eprintln(&eyre!("Too many request, waiting for {secs} s"));
                thread::sleep(Duration::from_secs(secs));
                send_get_request_rec(url, bounce + 1)
            } else {
                Ok(e)
            }
        })
}
