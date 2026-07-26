use std::collections::{HashSet, VecDeque};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Url;
use serde_json::{json, Value};

use crate::core::models::{VideoInfo, VideoPlaylistPreview};

const YOUTUBE_ORIGIN: &str = "https://www.youtube.com";
const MAX_CONTINUATION_PAGES: usize = 100;

pub async fn fetch_playlist(playlist_url: &str) -> Result<VideoPlaylistPreview> {
    let playlist_id = playlist_id_from_url(playlist_url)?;
    let canonical_url = format!("{YOUTUBE_ORIGIN}/playlist?list={playlist_id}");
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; CourseLib/0.7; +https://github.com/)")
        .build()
        .context("failed to build YouTube client")?;

    let html = client
        .get(&canonical_url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Cookie", "CONSENT=YES+cb")
        .send()
        .await
        .context("failed to fetch the YouTube playlist")?
        .error_for_status()
        .context("YouTube returned an error for this playlist")?
        .text()
        .await
        .context("failed to read the YouTube playlist response")?;

    let initial_data = extract_json_object_after_any(
        &html,
        &[
            "var ytInitialData =",
            "window[\"ytInitialData\"] =",
            "ytInitialData =",
        ],
    )
    .context("YouTube did not return playlist data; the playlist may be private or unavailable")?;
    let initial: Value = serde_json::from_str(initial_data)
        .context("failed to parse playlist data returned by YouTube")?;

    let title = find_playlist_title(&initial).unwrap_or_else(|| "YouTube playlist".to_string());
    let mut videos = Vec::new();
    let mut continuations = Vec::new();
    collect_playlist_data(&initial, &mut videos, &mut continuations);

    if !continuations.is_empty() {
        let api_key = extract_json_string(&html, "INNERTUBE_API_KEY")
            .context("YouTube did not provide the key needed to fetch the complete playlist")?;
        let client_version = extract_json_string(&html, "INNERTUBE_CLIENT_VERSION").context(
            "YouTube did not provide the client version needed to fetch the complete playlist",
        )?;
        fetch_continuations(
            &client,
            &api_key,
            &client_version,
            continuations,
            &mut videos,
        )
        .await?;
    }

    deduplicate_videos(&mut videos);
    if videos.is_empty() {
        bail!("no videos were found in this playlist; it may be empty, private, or unavailable");
    }

    Ok(VideoPlaylistPreview {
        playlist_id,
        playlist_title: title,
        playlist_url: canonical_url,
        video_count: videos.len(),
        videos,
    })
}

async fn fetch_continuations(
    client: &reqwest::Client,
    api_key: &str,
    client_version: &str,
    initial_tokens: Vec<String>,
    videos: &mut Vec<VideoInfo>,
) -> Result<()> {
    let mut queue = VecDeque::from(initial_tokens);
    let mut seen = HashSet::new();
    let endpoint = format!("{YOUTUBE_ORIGIN}/youtubei/v1/browse?key={api_key}");
    let mut pages = 0;

    while let Some(token) = queue.pop_front() {
        if !seen.insert(token.clone()) {
            continue;
        }
        if pages >= MAX_CONTINUATION_PAGES {
            bail!("playlist is too large to import safely");
        }
        pages += 1;

        let response = client
            .post(&endpoint)
            .header("Origin", YOUTUBE_ORIGIN)
            .header("Accept-Language", "en-US,en;q=0.9")
            .json(&json!({
                "context": {
                    "client": {
                        "clientName": "WEB",
                        "clientVersion": client_version
                    }
                },
                "continuation": token
            }))
            .send()
            .await
            .context("failed while fetching additional playlist videos")?
            .error_for_status()
            .context("YouTube returned an error while fetching additional playlist videos")?
            .json::<Value>()
            .await
            .context("failed to parse additional playlist videos")?;

        let mut next_tokens = Vec::new();
        collect_playlist_data(&response, videos, &mut next_tokens);
        queue.extend(next_tokens);
    }

    Ok(())
}

pub fn playlist_id_from_url(input: &str) -> Result<String> {
    let parsed = Url::parse(input.trim()).context("enter a valid YouTube playlist URL")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("YouTube playlist URL must include a host"))?
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if host != "youtube.com" && host != "m.youtube.com" && host != "music.youtube.com" {
        bail!("only YouTube playlist URLs are supported");
    }

    let playlist_id = parsed
        .query_pairs()
        .find(|(key, _)| key == "list")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("YouTube URL must include a playlist ID in the `list` parameter"))?;

    if !playlist_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        bail!("YouTube playlist ID contains invalid characters");
    }
    Ok(playlist_id)
}

fn collect_playlist_data(value: &Value, videos: &mut Vec<VideoInfo>, tokens: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(renderer) = object.get("playlistVideoRenderer") {
                if let Some(video) = parse_video_renderer(renderer) {
                    videos.push(video);
                }
            }
            // YouTube's current desktop response uses view models instead of the
            // older playlistVideoRenderer shape. Keep both parsers for compatibility.
            if let Some(view_model) = object.get("lockupViewModel") {
                if let Some(video) = parse_lockup_view_model(view_model) {
                    videos.push(video);
                }
            }
            if let Some(token) = object
                .get("continuationCommand")
                .and_then(|command| command.get("token"))
                .and_then(Value::as_str)
            {
                tokens.push(token.to_string());
            }
            for child in object.values() {
                collect_playlist_data(child, videos, tokens);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_playlist_data(item, videos, tokens);
            }
        }
        _ => {}
    }
}

fn parse_lockup_view_model(view_model: &Value) -> Option<VideoInfo> {
    if view_model.get("contentType").and_then(Value::as_str) != Some("LOCKUP_CONTENT_TYPE_VIDEO") {
        return None;
    }
    let video_id = view_model.get("contentId")?.as_str()?.to_string();
    let title = view_model
        .pointer("/metadata/lockupMetadataViewModel/title/content")
        .and_then(Value::as_str)
        .unwrap_or("Untitled video")
        .to_string();
    let duration = find_nested_string(view_model, "thumbnailBadgeViewModel", "text");
    let duration_seconds = duration.as_deref().and_then(parse_duration_seconds);
    let thumbnail_url = view_model
        .pointer("/contentImage/thumbnailViewModel/image/sources")
        .and_then(Value::as_array)
        .and_then(|sources| sources.last())
        .and_then(|source| source.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")));

    Some(video_info(
        video_id,
        title,
        duration,
        duration_seconds,
        thumbnail_url,
    ))
}

fn parse_video_renderer(renderer: &Value) -> Option<VideoInfo> {
    let video_id = renderer.get("videoId")?.as_str()?.to_string();
    let title = text_value(renderer.get("title")?).unwrap_or_else(|| "Untitled video".to_string());
    let duration = renderer.get("lengthText").and_then(text_value).or_else(|| {
        renderer
            .get("lengthSeconds")
            .and_then(Value::as_str)
            .map(format_seconds)
    });
    let duration_seconds = renderer
        .get("lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let thumbnail_url = renderer
        .pointer("/thumbnail/thumbnails")
        .and_then(Value::as_array)
        .and_then(|thumbnails| thumbnails.last())
        .and_then(|thumbnail| thumbnail.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")));

    Some(video_info(
        video_id,
        title,
        duration,
        duration_seconds,
        thumbnail_url,
    ))
}

fn video_info(
    video_id: String,
    title: String,
    duration: Option<String>,
    duration_seconds: Option<u64>,
    thumbnail_url: Option<String>,
) -> VideoInfo {
    VideoInfo {
        url: format!("https://www.youtube.com/watch?v={video_id}"),
        embed_url: format!("https://www.youtube-nocookie.com/embed/{video_id}"),
        video_id,
        title,
        duration,
        duration_seconds,
        thumbnail_url,
    }
}

fn find_nested_string(value: &Value, object_key: &str, value_key: &str) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(text) = object
                .get(object_key)
                .and_then(|nested| nested.get(value_key))
                .and_then(Value::as_str)
            {
                return Some(text.to_string());
            }
            object
                .values()
                .find_map(|child| find_nested_string(child, object_key, value_key))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|child| find_nested_string(child, object_key, value_key)),
        _ => None,
    }
}

fn parse_duration_seconds(duration: &str) -> Option<u64> {
    let mut total = 0u64;
    for part in duration.split(':') {
        total = total
            .checked_mul(60)?
            .checked_add(part.parse::<u64>().ok()?)?;
    }
    Some(total)
}

fn text_value(value: &Value) -> Option<String> {
    if let Some(text) = value.get("simpleText").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    let text = value
        .get("runs")?
        .as_array()?
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn find_playlist_title(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(metadata) = object.get("playlistMetadataRenderer") {
                if let Some(title) = metadata.get("title").and_then(Value::as_str) {
                    return Some(title.to_string());
                }
            }
            if let Some(header) = object.get("playlistHeaderRenderer") {
                if let Some(title) = header.get("title").and_then(text_value) {
                    return Some(title);
                }
            }
            object.values().find_map(find_playlist_title)
        }
        Value::Array(items) => items.iter().find_map(find_playlist_title),
        _ => None,
    }
}

fn deduplicate_videos(videos: &mut Vec<VideoInfo>) {
    let mut seen = HashSet::new();
    videos.retain(|video| seen.insert(video.video_id.clone()));
}

fn extract_json_string(html: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = html.find(&marker)? + marker.len();
    let tail = &html[start..];
    let mut escaped = false;
    for (index, character) in tail.char_indices() {
        if character == '"' && !escaped {
            return serde_json::from_str::<String>(&format!("\"{}\"", &tail[..index])).ok();
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    None
}

fn extract_json_object_after_any<'a>(html: &'a str, markers: &[&str]) -> Option<&'a str> {
    for marker in markers {
        let Some(marker_index) = html.find(marker) else {
            continue;
        };
        let tail = &html[marker_index + marker.len()..];
        let Some(object_start) = tail.find('{') else {
            continue;
        };
        let json = &tail[object_start..];
        if let Some(length) = balanced_json_object_length(json) {
            return Some(&json[..length]);
        }
    }
    None
}

fn balanced_json_object_length(input: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in input.bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn format_seconds(value: &str) -> String {
    let seconds = value.parse::<u64>().unwrap_or(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_playlist_and_watch_urls() {
        assert_eq!(
            playlist_id_from_url("https://www.youtube.com/playlist?list=PLabc-_123").unwrap(),
            "PLabc-_123"
        );
        assert_eq!(
            playlist_id_from_url("https://youtube.com/watch?v=video&list=PLcourse").unwrap(),
            "PLcourse"
        );
    }

    #[test]
    fn parses_current_lockup_view_model_shape() {
        let value = json!({
            "lockupViewModel": {
                "contentId": "current",
                "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
                "metadata": {"lockupMetadataViewModel": {"title": {"content": "Current title"}}},
                "contentImage": {"thumbnailViewModel": {
                    "image": {"sources": [{"url": "https://example.com/thumb.jpg"}]},
                    "overlays": [{"thumbnailBottomOverlayViewModel": {"badges": [
                        {"thumbnailBadgeViewModel": {"text": "1:02:03"}}
                    ]}}]
                }}
            }
        });
        let mut videos = Vec::new();
        let mut tokens = Vec::new();
        collect_playlist_data(&value, &mut videos, &mut tokens);
        assert_eq!(videos[0].video_id, "current");
        assert_eq!(videos[0].title, "Current title");
        assert_eq!(videos[0].duration_seconds, Some(3723));
    }

    #[test]
    fn rejects_non_youtube_and_missing_playlist_ids() {
        assert!(playlist_id_from_url("https://example.com/?list=PLabc").is_err());
        assert!(playlist_id_from_url("https://youtube.com/watch?v=abc").is_err());
    }

    #[test]
    fn extracts_balanced_initial_data() {
        let html =
            r#"<script>var ytInitialData = {"text":"a } brace","nested":{"ok":true}};</script>"#;
        let json = extract_json_object_after_any(html, &["var ytInitialData ="]).unwrap();
        let value: Value = serde_json::from_str(json).unwrap();
        assert_eq!(value.pointer("/nested/ok"), Some(&Value::Bool(true)));
    }

    #[test]
    fn collects_videos_in_array_order() {
        let value = json!({"contents": [
            {"playlistVideoRenderer": {"videoId": "one", "title": {"simpleText": "First"}, "lengthSeconds": "61"}},
            {"playlistVideoRenderer": {"videoId": "two", "title": {"runs": [{"text": "Second"}]}}}
        ]});
        let mut videos = Vec::new();
        let mut tokens = Vec::new();
        collect_playlist_data(&value, &mut videos, &mut tokens);
        assert_eq!(
            videos
                .iter()
                .map(|video| video.video_id.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        assert_eq!(videos[0].duration.as_deref(), Some("1:01"));
    }
}
