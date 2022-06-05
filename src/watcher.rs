use crate::{
    discord::WebhookClient,
    twitch::{Error, Stream, TwitchClient},
};

struct StreamSegment;

pub enum StreamUpdate {
    LIVE(Stream),
    OFFLINE,
}

pub struct StreamWatcher {
    user_login: String,
    segments: Vec<StreamSegment>,
}

impl StreamWatcher {
    pub fn new(user_login: String) -> Self {
        Self {
            user_login,
            segments: Vec::new(),
        }
    }

    pub async fn update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: StreamUpdate,
    ) -> Result<(), Error> {
        // TODO: implement logic for LIVE and UPDATE
        match stream {
            StreamUpdate::OFFLINE => self.on_offline(client, webhook).await,
            StreamUpdate::LIVE(stream) if self.segments.is_empty() => {
                self.on_go_live(client, webhook, stream).await
            }
            StreamUpdate::LIVE(stream) => self.on_update(client, webhook, stream).await,
        }
    }

    async fn on_go_live(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_offline(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
    ) -> Result<(), Error> {
        Ok(())
    }
}
