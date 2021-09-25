use serenity::{model::channel::Message, Result as SerenityResult};
use songbird::tracks::TrackHandle;

/// Checks that a message successfully sent; if not, then logs why to stdout.
pub fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

pub fn format_track(track: &TrackHandle, format: bool) -> String {
    let artist = track
        .metadata()
        .artist
        .as_ref()
        .unwrap_or(&"Unknown".to_owned())
        .to_owned();
    let title = track
        .metadata()
        .title
        .as_ref()
        .unwrap_or(&"Unknown".to_owned())
        .to_owned();

    let raw = format!("{} - {}", artist, title);

    if format {
        format!(
            "**{}**",
            raw.replace("*", "\\*")
                .replace("_", "\\_")
                .replace("~", "\\~")
                .replace("`", "")
        )
    } else {
        raw
    }
}
