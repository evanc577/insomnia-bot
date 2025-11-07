#![allow(dead_code)]

use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{PoiseContext, CLIENT};

static AUTOCOMPLETE_ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/music/get_search_suggestions?key=AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30&prettyPrint=false";
static AUTOCOMPLETE_CLIENT_NAME: &str = "WEB_REMIX";
static AUTOCOMPLETE_CLIENT_VERSION: &str = "1.20220919.01.00";

#[derive(Serialize, Default)]
struct RequestBody {
    input: String,
    context: RequestContext,
}
#[derive(Serialize, Default)]
struct RequestContext {
    client: RequestClient,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestClient {
    client_name: String,
    client_version: String,
}

impl Default for RequestClient {
    fn default() -> Self {
        Self {
            client_name: AUTOCOMPLETE_CLIENT_NAME.into(),
            client_version: AUTOCOMPLETE_CLIENT_VERSION.into(),
        }
    }
}

#[derive(Deserialize)]
struct ResponseBody {
    contents: Vec<ResponseContent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseContent {
    search_suggestions_section_renderer: ResponseSearchSuggestionsSectionRenderer,
}

#[derive(Deserialize)]
struct ResponseSearchSuggestionsSectionRenderer {
    contents: Vec<ResponseContentInner>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseContentInner {
    search_suggestion_renderer: ResponseSearchSuggestionRenderer,
}

#[derive(Deserialize)]
struct ResponseSearchSuggestionRenderer {
    suggestion: ResponseSuggestion,
}

#[derive(Deserialize)]
struct ResponseSuggestion {
    runs: Vec<ResponseRun>,
}

#[derive(Deserialize)]
struct ResponseRun {
    text: String,
}

pub async fn autocomplete_ytmusic(_ctx: PoiseContext<'_>, partial: &str) -> Vec<String> {
    autocomplete_ytmusic_helper(partial)
        .await
        .unwrap_or_default()
}

async fn autocomplete_ytmusic_helper(partial: &str) -> Result<Vec<String>> {
    let json = RequestBody {
        input: partial.into(),
        ..Default::default()
    };
    let suggestions = CLIENT
        .post(AUTOCOMPLETE_ENDPOINT)
        .header("Accept", "*/*")
        .header("Content-Type", "application/json")
        .header("Origin", "https://music.youtube.com")
        .header("Connection", "keep-alive")
        .json(&json)
        .send()
        .await?
        .json::<ResponseBody>()
        .await?
        .contents
        .first()
        .ok_or_else(|| anyhow::anyhow!("no suggestions"))?
        .search_suggestions_section_renderer
        .contents
        .iter()
        .map(|rci| {
            rci.search_suggestion_renderer
                .suggestion
                .runs
                .iter()
                .map(|r| r.text.as_str())
                .join("")
        })
        .collect();
    Ok(suggestions)
}
