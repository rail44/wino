use atom_syndication::{Entry, Feed as AtomFeed};
use chrono::{DateTime, FixedOffset};
use js_sys::Date;
use rss::extension::dublincore::DublinCoreExtension;
use rss::{Channel, Item};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub new_feed_url: String,
    pub is_loading_new_feed: bool,
    pub feed_map: HashMap<String, Feed>,
}

impl Default for State {
    fn default() -> State {
        State {
            new_feed_url: String::new(),
            is_loading_new_feed: false,
            feed_map: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Feed {
    pub title: String,
    pub url: String,
    pub article_map: HashMap<String, Article>,
    pub updated: f64,
    pub visible: bool,
}

impl Default for Feed {
    fn default() -> Self {
        Feed {
            title: String::default(),
            url: String::default(),
            article_map: HashMap::default(),
            updated: Date::now(),
            visible: true,
        }
    }
}

impl Feed {
    pub fn from_atom(url: String, atom: &AtomFeed) -> Self {
        let mut article_map = HashMap::new();

        for entry in atom.entries() {
            let id = entry.id();
            article_map.insert(id.to_string(), Article::from_atom(entry));
        }

        Feed {
            article_map,
            url,
            title: atom.title().to_string(),
            ..Default::default()
        }
    }

    pub fn from_rss(url: String, channel: &Channel) -> Self {
        let mut article_map = HashMap::default();
        for item in channel.items() {
            let article = Article::from_rss(item);
            let id = item
                .guid()
                .map_or_else(|| article.url.clone(), |guid| guid.value().to_string());
            article_map.insert(id, article);
        }

        Feed {
            article_map,
            url,
            title: channel.title().to_string(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub title: String,
    pub date: DateTime<FixedOffset>,
    pub url: String,
}

impl Article {
    fn from_atom(entry: &Entry) -> Self {
        let date = parse_date(entry.published().unwrap_or(""));
        Article {
            title: entry.title().to_string(),
            url: entry
                .links()
                .get(0)
                .map_or("", |link| link.href())
                .to_string(),
            date,
        }
    }

    fn from_rss(item: &Item) -> Self {
        let date_str = item
            .pub_date()
            .or_else(|| {
                item.dublin_core_ext()
                    .map(DublinCoreExtension::dates)
                    .and_then(|date| date.get(0))
                    .map(String::as_str)
            })
            .unwrap_or("");
        let date = parse_date(date_str);
        let url = item.link().unwrap_or("").to_string();
        Article {
            title: item.title().unwrap_or("").to_string(),
            url: url.clone(),
            date,
        }
    }
}

fn parse_date(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_rfc2822(s))
        .unwrap()
}
