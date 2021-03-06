use crate::errors::*;
use crate::sync::NewStatus;
use egg_mode::media::{set_metadata, upload_media};
use egg_mode::tweet::DraftTweet;
use egg_mode::tweet::Tweet;
use egg_mode::Token;
use failure::format_err;
use mammut::entities::status::Status;
use mammut::media_builder::MediaBuilder;
use mammut::status_builder::StatusBuilder;
use mammut::Mastodon;
use reqwest::header::CONTENT_TYPE;
use std::fs::remove_file;
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::prelude::*;

pub async fn post_to_mastodon(mastodon: &Mastodon, toot: NewStatus) -> Result<Status> {
    let mut status = StatusBuilder::new(toot.text.clone());
    // Post attachments first, if there are any.
    for attachment in toot.attachments {
        // Because we use async for egg-mode we also need to use reqwest in
        // async mode. Otherwise we get double async executor errors.
        let response = reqwest::get(&attachment.attachment_url)
            .await
            .context(format!(
                "Failed downloading attachment {}",
                attachment.attachment_url
            ))?;
        let tmpfile = NamedTempFile::new()?;

        // Oh boy, this looks really bad. I could not use the path directly because
        // the compiler would not let me. Can this be simpler?
        let path = tmpfile.path().to_str().unwrap().to_string();

        let mut file = File::create(&path).await?;
        file.write_all(&response.bytes().await?).await?;

        let attachment = match attachment.alt_text {
            None => mastodon.media(path.into())?,
            Some(description) => mastodon.media(MediaBuilder {
                file: path.into(),
                description: Some(description.into()),
                focus: None,
            })?,
        };

        match status.media_ids.as_mut() {
            Some(ids) => {
                ids.push(attachment.id);
            }
            None => {
                status.media_ids = Some(vec![attachment.id]);
            }
        }
        remove_file(tmpfile)?;
    }

    match mastodon.new_status(status) {
        Ok(s) => Ok(s),
        Err(e) => Err(e.into()),
    }
}

/// Send a new status update to Twitter, including attachments.
pub async fn post_to_twitter(token: &Token, tweet: NewStatus) -> Result<Tweet> {
    let mut draft = DraftTweet::new(tweet.text);
    for attachment in tweet.attachments {
        let response = reqwest::get(&attachment.attachment_url).await?;
        let media_type = response
            .headers()
            .get(CONTENT_TYPE)
            .ok_or_else(|| format_err!("Missing content-type on response"))?
            .to_str()?
            .parse::<mime::Mime>()?;
        let bytes = response.bytes().await?;
        let media_handle = upload_media(&bytes, &media_type, &token).await?;
        draft.add_media(media_handle.id.clone());
        if let Some(alt_text) = attachment.alt_text {
            set_metadata(&media_handle.id, &alt_text, &token).await?;
        }
    }
    let created_tweet = draft.send(&token).await?;
    Ok((*created_tweet).clone())
}
