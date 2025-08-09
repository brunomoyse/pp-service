use async_graphql::{Subscription};
use futures_util::Stream;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Simple ticking subscription (1..∞), useful as a template for live clock/announcements.
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut i = 0;
        IntervalStream::new(interval(Duration::from_secs(1)))
            .map(move |_| {
                i += 1;
                i
            })
    }
}