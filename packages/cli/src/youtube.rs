//! YouTube video capture via the `yt-dlp` CLI.
//!
//! A video page has no article to extract: the content is the video's
//! description, chapters and transcript. YouTube no longer serves caption
//! tracks to plain HTTP clients, so this module asks `yt-dlp` (if installed)
//! for the metadata and captions and renders them as markdown.
//!
//! Everything here is best-effort. When `yt-dlp` is missing or fails, the
//! caller falls back to the normal page save.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, instrument};
use url::Url;

use crate::agent::provider::ProcessRunner;

/// Seconds of speech grouped into one transcript paragraph.
const PARAGRAPH_SECONDS: u64 = 60;

#[derive(Debug, thiserror::Error)]
pub enum YoutubeError {
    #[error("failed to run yt-dlp: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("yt-dlp exited with status {status}: {stderr}")]
    Failed { status: i32, stderr: String },

    #[error("yt-dlp produced unreadable metadata: {0}")]
    InvalidMetadata(String),

    #[error("could not prepare a working directory for yt-dlp: {0}")]
    WorkDir(#[source] std::io::Error),
}

/// A captured video.
#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    pub id: String,
    pub url: String,
    pub title: String,
    pub channel: Option<String>,
    /// `YYYY-MM-DD`.
    pub published: Option<String>,
    pub duration_seconds: Option<u64>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub chapters: Vec<Chapter>,
    pub transcript: Option<Transcript>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub start_seconds: u64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    /// True when the captions were machine-generated.
    pub auto_generated: bool,
    pub lines: Vec<TranscriptLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptLine {
    pub start_seconds: u64,
    pub text: String,
}

// ── URL recognition ─────────────────────────────────────────────────

/// Extract the video ID from a YouTube video URL.
///
/// Recognises `watch?v=`, `youtu.be/`, `/shorts/`, `/live/` and `/embed/`.
/// Channel, playlist and search pages return `None`.
pub fn video_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);

    let candidate = if host == "youtu.be" {
        parsed.path_segments()?.next().map(str::to_string)
    } else if host == "youtube.com" || host == "m.youtube.com" || host == "music.youtube.com" {
        let mut segments = parsed.path_segments()?;
        match segments.next() {
            Some("watch") => parsed
                .query_pairs()
                .find(|(key, _)| key == "v")
                .map(|(_, value)| value.into_owned()),
            Some("shorts" | "live" | "embed") => segments.next().map(str::to_string),
            _ => None,
        }
    } else {
        None
    }?;

    is_video_id(&candidate).then_some(candidate)
}

fn is_video_id(candidate: &str) -> bool {
    candidate.len() == 11
        && candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The canonical watch URL for a video ID.
pub fn watch_url(id: &str) -> String {
    format!("https://www.youtube.com/watch?v={id}")
}

// ── Fetching ────────────────────────────────────────────────────────

/// Fetch a video's metadata and English captions with `yt-dlp`.
#[instrument(skip(runner))]
pub(crate) fn fetch_video(id: &str, runner: &dyn ProcessRunner) -> Result<Video, YoutubeError> {
    let work_dir = WorkDir::create()?;
    let dir = work_dir.path().to_string_lossy().to_string();
    let url = watch_url(id);

    let args = [
        "--skip-download",
        "--write-subs",      // captions written by the uploader, when present
        "--write-auto-subs", // otherwise the machine-generated ones
        "--sub-langs",
        "en.*,en",
        "--sub-format",
        "json3",
        "--no-playlist",
        "--no-warnings",
        "-P",
        &dir,
        "-o",
        "%(id)s",
        "--dump-single-json", // metadata on stdout
        "--no-simulate",      // …and still write the caption files
        &url,
    ];
    let output = runner
        .run("yt-dlp", &args, "")
        .map_err(YoutubeError::Spawn)?;
    if output.exit_code != 0 {
        let stderr: String = output.stderr.trim().chars().take(500).collect();
        return Err(YoutubeError::Failed {
            status: output.exit_code,
            stderr,
        });
    }

    let meta: RawMetadata = serde_json::from_str(output.stdout.trim())
        .map_err(|e| YoutubeError::InvalidMetadata(e.to_string()))?;
    let transcript = read_transcript(work_dir.path(), id, &meta);
    debug!(
        title = %meta.title,
        transcript_lines = transcript.as_ref().map(|t| t.lines.len()),
        "video fetched"
    );

    Ok(Video {
        id: id.to_string(),
        url,
        title: meta.title,
        channel: meta.channel.or(meta.uploader),
        published: meta.upload_date.as_deref().and_then(format_upload_date),
        duration_seconds: meta.duration.map(|d| d.max(0.0) as u64),
        description: meta
            .description
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty()),
        thumbnail_url: meta.thumbnail,
        chapters: meta
            .chapters
            .unwrap_or_default()
            .into_iter()
            .map(|c| Chapter {
                start_seconds: c.start_time.max(0.0) as u64,
                title: c.title,
            })
            .collect(),
        transcript,
    })
}

#[derive(Deserialize)]
struct RawMetadata {
    title: String,
    channel: Option<String>,
    uploader: Option<String>,
    /// `YYYYMMDD`.
    upload_date: Option<String>,
    duration: Option<f64>,
    description: Option<String>,
    thumbnail: Option<String>,
    chapters: Option<Vec<RawChapter>>,
    /// Uploader-written caption tracks by language. Present ⇒ not auto-generated.
    subtitles: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct RawChapter {
    start_time: f64,
    title: String,
}

fn format_upload_date(raw: &str) -> Option<String> {
    (raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..]))
}

/// Pick and parse the best caption file `yt-dlp` wrote for this video.
fn read_transcript(dir: &Path, id: &str, meta: &RawMetadata) -> Option<Transcript> {
    let prefix = format!("{id}.");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".json3"))
        })
        .collect();
    // Plain `en` first, then the remaining English variants in a stable order.
    files.sort_by_key(|path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        (name != format!("{id}.en.json3"), name.to_string())
    });

    let has_manual_english = meta
        .subtitles
        .as_ref()
        .is_some_and(|subs| subs.keys().any(|lang| lang.starts_with("en")));

    files.iter().find_map(|path| {
        let raw = std::fs::read_to_string(path).ok()?;
        let lines = parse_json3(&raw)?;
        (!lines.is_empty()).then_some(Transcript {
            auto_generated: !has_manual_english,
            lines,
        })
    })
}

#[derive(Deserialize)]
struct Json3 {
    #[serde(default)]
    events: Vec<Json3Event>,
}

#[derive(Deserialize)]
struct Json3Event {
    #[serde(rename = "tStartMs", default)]
    start_ms: u64,
    #[serde(default)]
    segs: Vec<Json3Segment>,
}

#[derive(Deserialize)]
struct Json3Segment {
    #[serde(default)]
    utf8: String,
}

/// Parse YouTube's `json3` caption format into timed lines of text.
fn parse_json3(raw: &str) -> Option<Vec<TranscriptLine>> {
    let parsed: Json3 = serde_json::from_str(raw).ok()?;
    Some(
        parsed
            .events
            .into_iter()
            .filter_map(|event| {
                let text: String = event.segs.iter().map(|seg| seg.utf8.as_str()).collect();
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                (!text.is_empty()).then_some(TranscriptLine {
                    start_seconds: event.start_ms / 1000,
                    text,
                })
            })
            .collect(),
    )
}

/// Temporary directory for one `yt-dlp` run, removed on drop.
struct WorkDir(PathBuf);

impl WorkDir {
    fn create() -> Result<Self, YoutubeError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("agentmark-youtube-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(YoutubeError::WorkDir)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Rendering ───────────────────────────────────────────────────────

/// `1:02:03` or `2:03`.
fn timestamp(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Render a video as a markdown document: details, description, transcript.
///
/// View counts and other fast-changing numbers are left out so that the
/// content hash only changes when the video's content does.
pub fn render_markdown(video: &Video) -> String {
    let mut out = format!("# {}\n\n", video.title);

    if let Some(ref channel) = video.channel {
        out.push_str(&format!("- Channel: {channel}\n"));
    }
    if let Some(ref published) = video.published {
        out.push_str(&format!("- Published: {published}\n"));
    }
    if let Some(duration) = video.duration_seconds {
        out.push_str(&format!("- Duration: {}\n", timestamp(duration)));
    }
    out.push_str(&format!("- Video: <{}>\n", video.url));

    if let Some(ref description) = video.description {
        out.push_str(&format!("\n## Description\n\n{description}\n"));
    }

    out.push_str("\n## Transcript\n\n");
    match video.transcript {
        Some(ref transcript) => {
            if transcript.auto_generated {
                out.push_str("_Auto-generated captions; expect transcription errors._\n\n");
            }
            out.push_str(&render_transcript(transcript, &video.chapters));
        }
        None => out.push_str("No English captions were available for this video.\n"),
    }
    out
}

/// Transcript as timestamped paragraphs, with a heading at each chapter start.
fn render_transcript(transcript: &Transcript, chapters: &[Chapter]) -> String {
    let mut out = String::new();
    let mut next_chapter = 0;
    let mut paragraph = String::new();
    let mut paragraph_start = 0;

    let flush = |out: &mut String, paragraph: &mut String, start: u64| {
        if !paragraph.is_empty() {
            out.push_str(&format!(
                "**[{}]** {}\n\n",
                timestamp(start),
                paragraph.trim()
            ));
            paragraph.clear();
        }
    };

    for line in &transcript.lines {
        let chapter_starts = next_chapter < chapters.len()
            && line.start_seconds >= chapters[next_chapter].start_seconds;
        if chapter_starts {
            flush(&mut out, &mut paragraph, paragraph_start);
            // Skip chapters with no speech of their own.
            while next_chapter + 1 < chapters.len()
                && line.start_seconds >= chapters[next_chapter + 1].start_seconds
            {
                next_chapter += 1;
            }
            out.push_str(&format!("### {}\n\n", chapters[next_chapter].title));
            next_chapter += 1;
        } else if !paragraph.is_empty() && line.start_seconds >= paragraph_start + PARAGRAPH_SECONDS
        {
            flush(&mut out, &mut paragraph, paragraph_start);
        }

        if paragraph.is_empty() {
            paragraph_start = line.start_seconds;
        } else {
            paragraph.push(' ');
        }
        paragraph.push_str(&line.text);
    }
    flush(&mut out, &mut paragraph, paragraph_start);
    out
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::provider::ProcessOutput;
    use std::io;

    // ── URL recognition ─────────────────────────────────────────────

    #[test]
    fn recognises_video_urls() {
        for url in [
            "https://www.youtube.com/watch?v=zjkBMFhNj_g",
            "https://youtube.com/watch?feature=share&v=zjkBMFhNj_g&t=42s",
            "https://m.youtube.com/watch?v=zjkBMFhNj_g",
            "https://youtu.be/zjkBMFhNj_g?si=abc",
            "https://www.youtube.com/shorts/zjkBMFhNj_g",
            "https://www.youtube.com/live/zjkBMFhNj_g",
            "https://www.youtube.com/embed/zjkBMFhNj_g",
        ] {
            assert_eq!(video_id(url).as_deref(), Some("zjkBMFhNj_g"), "{url}");
        }
    }

    #[test]
    fn ignores_non_video_urls() {
        for url in [
            "https://www.youtube.com/",
            "https://www.youtube.com/@AndrejKarpathy",
            "https://www.youtube.com/playlist?list=PLabc",
            "https://www.youtube.com/results?search_query=llm",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/watch?v=has%20space!",
            "https://notyoutube.com/watch?v=zjkBMFhNj_g",
            "https://example.com/watch?v=zjkBMFhNj_g",
            "not a url",
        ] {
            assert_eq!(video_id(url), None, "{url}");
        }
    }

    #[test]
    fn watch_url_is_canonical() {
        assert_eq!(
            watch_url("zjkBMFhNj_g"),
            "https://www.youtube.com/watch?v=zjkBMFhNj_g"
        );
    }

    // ── Fetching with a fake yt-dlp ─────────────────────────────────

    const ID: &str = "zjkBMFhNj_g";

    /// Stands in for yt-dlp: writes caption files into the `-P` directory and
    /// prints metadata, or fails.
    struct FakeYtDlp {
        metadata: String,
        captions: Vec<(&'static str, String)>,
        exit_code: i32,
        spawn_fails: bool,
    }

    impl ProcessRunner for FakeYtDlp {
        fn run(
            &self,
            program: &str,
            args: &[&str],
            _stdin: &str,
        ) -> Result<ProcessOutput, io::Error> {
            assert_eq!(program, "yt-dlp");
            assert!(args.contains(&"--skip-download"));
            assert_eq!(args.last(), Some(&watch_url(ID).as_str()));
            if self.spawn_fails {
                return Err(io::Error::new(io::ErrorKind::NotFound, "no such program"));
            }
            let dir = args[args.iter().position(|a| *a == "-P").unwrap() + 1];
            for (suffix, body) in &self.captions {
                std::fs::write(Path::new(dir).join(format!("{ID}.{suffix}.json3")), body).unwrap();
            }
            Ok(ProcessOutput {
                stdout: self.metadata.clone(),
                stderr: "ERROR: something broke".to_string(),
                exit_code: self.exit_code,
            })
        }
    }

    fn metadata(extra: &str) -> String {
        format!(
            r#"{{"title":"Intro to LLMs","channel":"Andrej Karpathy","uploader":"ak",
"upload_date":"20231123","duration":3588,"description":"  A talk.\n\nLinks below.  ",
"thumbnail":"https://i.ytimg.com/vi/x/maxres.jpg","view_count":4349826{extra}}}"#
        )
    }

    fn captions(lines: &[(u64, &str)]) -> String {
        let events: Vec<String> = lines
            .iter()
            .map(|(ms, text)| {
                format!(
                    r#"{{"tStartMs":{ms},"dDurationMs":1000,"segs":[{{"utf8":{}}}]}}"#,
                    serde_json::json!(text)
                )
            })
            .collect();
        format!(
            r#"{{"wireMagic":"pb3","events":[{{"tStartMs":0,"id":1}},{}]}}"#,
            events.join(",")
        )
    }

    fn fake(metadata: String, captions: Vec<(&'static str, String)>) -> FakeYtDlp {
        FakeYtDlp {
            metadata,
            captions,
            exit_code: 0,
            spawn_fails: false,
        }
    }

    #[test]
    fn fetches_metadata_and_auto_captions() {
        let runner = fake(
            metadata(""),
            vec![(
                "en",
                captions(&[
                    (0, "hi everyone"),
                    (1500, "\n"),
                    (2000, "so  recently\nI gave"),
                ]),
            )],
        );

        let video = fetch_video(ID, &runner).unwrap();

        assert_eq!(video.title, "Intro to LLMs");
        assert_eq!(video.channel.as_deref(), Some("Andrej Karpathy"));
        assert_eq!(video.published.as_deref(), Some("2023-11-23"));
        assert_eq!(video.duration_seconds, Some(3588));
        assert_eq!(
            video.description.as_deref(),
            Some("A talk.\n\nLinks below.")
        );
        assert_eq!(video.url, watch_url(ID));
        let transcript = video.transcript.unwrap();
        assert!(transcript.auto_generated);
        assert_eq!(
            transcript.lines,
            vec![
                TranscriptLine {
                    start_seconds: 0,
                    text: "hi everyone".to_string()
                },
                TranscriptLine {
                    start_seconds: 2,
                    text: "so recently I gave".to_string()
                },
            ]
        );
    }

    #[test]
    fn uploader_captions_are_not_marked_auto_generated() {
        let runner = fake(
            metadata(r#","subtitles":{"en":[{"ext":"json3"}]}"#),
            vec![("en", captions(&[(0, "Hello.")]))],
        );
        assert!(
            !fetch_video(ID, &runner)
                .unwrap()
                .transcript
                .unwrap()
                .auto_generated
        );
    }

    #[test]
    fn prefers_plain_english_over_variants() {
        let runner = fake(
            metadata(""),
            vec![
                ("en-orig", captions(&[(0, "variant")])),
                ("en", captions(&[(0, "plain")])),
            ],
        );
        let transcript = fetch_video(ID, &runner).unwrap().transcript.unwrap();
        assert_eq!(transcript.lines[0].text, "plain");
    }

    #[test]
    fn video_without_captions_has_no_transcript() {
        let video = fetch_video(ID, &fake(metadata(""), vec![])).unwrap();
        assert!(video.transcript.is_none());
        let empty = fake(metadata(""), vec![("en", r#"{"events":[]}"#.to_string())]);
        assert!(fetch_video(ID, &empty).unwrap().transcript.is_none());
    }

    #[test]
    fn falls_back_to_uploader_when_channel_is_missing() {
        let meta = r#"{"title":"T","uploader":"someone"}"#.to_string();
        let video = fetch_video(ID, &fake(meta, vec![])).unwrap();
        assert_eq!(video.channel.as_deref(), Some("someone"));
        assert_eq!(video.published, None);
        assert_eq!(video.description, None);
    }

    #[test]
    fn reports_missing_program_failure_and_bad_output() {
        let mut runner = fake(metadata(""), vec![]);
        runner.spawn_fails = true;
        assert!(matches!(
            fetch_video(ID, &runner),
            Err(YoutubeError::Spawn(_))
        ));

        let mut runner = fake(metadata(""), vec![]);
        runner.exit_code = 1;
        match fetch_video(ID, &runner) {
            Err(YoutubeError::Failed { status, stderr }) => {
                assert_eq!(status, 1);
                assert!(stderr.contains("something broke"));
            }
            other => panic!("unexpected: {other:?}"),
        }

        let runner = fake("not json".to_string(), vec![]);
        assert!(matches!(
            fetch_video(ID, &runner),
            Err(YoutubeError::InvalidMetadata(_))
        ));
    }

    // ── Rendering ───────────────────────────────────────────────────

    fn line(start_seconds: u64, text: &str) -> TranscriptLine {
        TranscriptLine {
            start_seconds,
            text: text.to_string(),
        }
    }

    fn sample_video() -> Video {
        Video {
            id: ID.to_string(),
            url: watch_url(ID),
            title: "Intro to LLMs".to_string(),
            channel: Some("Andrej Karpathy".to_string()),
            published: Some("2023-11-23".to_string()),
            duration_seconds: Some(3723),
            description: Some("A talk.".to_string()),
            thumbnail_url: None,
            chapters: vec![],
            transcript: Some(Transcript {
                auto_generated: true,
                lines: vec![
                    line(0, "hi everyone"),
                    line(30, "so recently"),
                    line(61, "next minute"),
                ],
            }),
        }
    }

    #[test]
    fn renders_details_description_and_timestamped_transcript() {
        let md = render_markdown(&sample_video());
        assert_eq!(
            md,
            "# Intro to LLMs\n\n\
             - Channel: Andrej Karpathy\n\
             - Published: 2023-11-23\n\
             - Duration: 1:02:03\n\
             - Video: <https://www.youtube.com/watch?v=zjkBMFhNj_g>\n\
             \n## Description\n\nA talk.\n\
             \n## Transcript\n\n\
             _Auto-generated captions; expect transcription errors._\n\n\
             **[0:00]** hi everyone so recently\n\n\
             **[1:01]** next minute\n\n"
        );
    }

    #[test]
    fn renders_chapter_headings_at_chapter_starts() {
        let mut video = sample_video();
        video.chapters = vec![
            Chapter {
                start_seconds: 0,
                title: "Intro".to_string(),
            },
            Chapter {
                start_seconds: 20,
                title: "Silent chapter".to_string(),
            },
            Chapter {
                start_seconds: 25,
                title: "LLM inference".to_string(),
            },
            Chapter {
                start_seconds: 4000,
                title: "Never reached".to_string(),
            },
        ];
        let md = render_markdown(&video);
        assert!(
            md.ends_with(
                "### Intro\n\n**[0:00]** hi everyone\n\n\
                 ### LLM inference\n\n**[0:30]** so recently next minute\n\n"
            ),
            "{md}"
        );
        assert!(!md.contains("Silent chapter"), "{md}");
        assert!(!md.contains("Never reached"), "{md}");
    }

    #[test]
    fn renders_video_without_captions_or_optional_details() {
        let mut video = sample_video();
        video.channel = None;
        video.published = None;
        video.duration_seconds = None;
        video.description = None;
        video.transcript = None;
        assert_eq!(
            render_markdown(&video),
            "# Intro to LLMs\n\n- Video: <https://www.youtube.com/watch?v=zjkBMFhNj_g>\n\
             \n## Transcript\n\nNo English captions were available for this video.\n"
        );
    }

    #[test]
    fn uploader_captions_have_no_auto_generated_notice() {
        let mut video = sample_video();
        video.transcript.as_mut().unwrap().auto_generated = false;
        assert!(!render_markdown(&video).contains("Auto-generated"));
    }

    #[test]
    fn timestamps_format_with_and_without_hours() {
        assert_eq!(timestamp(0), "0:00");
        assert_eq!(timestamp(75), "1:15");
        assert_eq!(timestamp(3723), "1:02:03");
    }
}
