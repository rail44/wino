#![feature(proc_macro_hygiene)]

extern crate atom_syndication;
extern crate chrono;
extern crate console_error_panic_hook;
extern crate futures;
extern crate rss;
extern crate serde;
extern crate serde_json;
extern crate squark;
extern crate squark_macros;
extern crate squark_web;
extern crate wasm_bindgen;
extern crate wasm_bindgen_futures;
extern crate web_sys;
extern crate js_sys;

use atom_syndication::{Feed as AtomFeed, Entry};
use chrono::{DateTime, FixedOffset};
use console_error_panic_hook::set_once as set_panic_hook;
use futures::Future;
use rss::{Channel, Item};
use squark::{App, Child, HandlerArg, Runtime, Task, View};
use squark_macros::view;
use squark_web::WebRuntime;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use web_sys::{console, window};
use js_sys::{Date};

mod fetch;

#[derive(Clone, Debug)]
enum Action {
    UpdateNewFeedUrl(String),
    AddFeed,
    Fetched(String, String),
    Reload,
}

#[derive(Clone, Debug, PartialEq)]
struct Feed {
    title: String,
    url: String,
    article_map: HashMap<String, Article>,
    updated: f64,
}

impl Default for Feed {
    fn default() -> Self {
        Feed {
            title: String::default(),
            url: String::default(),
            article_map: HashMap::default(),
            updated: Date::now(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Article {
    title: String,
    date: DateTime<FixedOffset>,
    url: String,
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
        let date_str = item.pub_date().or_else(|| {
            item.dublin_core_ext()
                .map(|dce| dce.dates())
                .and_then(|date| date.get(0))
                .map(|s| s.as_str())
        }).unwrap_or("");
        let date = parse_date(date_str);
        let url = item.link().unwrap_or("").to_string();
        Article {
            title: item.title().unwrap_or("").to_string(),
            url: url.clone(),
            date,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct State {
    new_feed_url: String,
    is_loading_new_feed: bool,
    feed_map: HashMap<String, Feed>,
}

impl Feed {
    fn from_atom(url: String, atom: &AtomFeed) -> Self {
        let mut article_map = HashMap::new();

        for entry in atom.entries() {
            let id = entry.id();
            article_map.insert(id.to_string(), Article::from_atom(entry));
        }

        Feed {
            article_map,
            title: atom.title().to_string(),
            url: url.clone(),
            updated: Date::now(),
        }
    }

    fn from_rss(url: String, channel: &Channel) -> Self {
        let mut article_map = HashMap::default();
        for item in channel.items() {
            let article = Article::from_rss(item);
            let id = item.guid().map_or_else(|| article.url.clone(), |guid| guid.value().to_string());
            article_map.insert(id, article);
        }

        Feed {
            article_map,
            title: channel.title().to_string(),
            url: url.clone(),
            updated: Date::now(),
        }
    }
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

fn parse_date(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_rfc2822(s))
        .unwrap()
}

#[derive(Clone, Debug)]
struct WinoApp;
impl App for WinoApp {
    type State = State;
    type Action = Action;

    fn reducer(&self, mut state: State, action: Action) -> (State, Task<Action>) {
        let mut task = Task::empty();
        match action {
            Action::UpdateNewFeedUrl(url) => {
                state.new_feed_url = url;
                (state, task)
            }
            Action::AddFeed => {
                let new_feed_url = state.new_feed_url.clone();
                let future = fetch::get(&state.new_feed_url)
                    .map(move |body| Action::Fetched(new_feed_url, body.as_string().unwrap()))
                    .map_err(|_| ());
                task.push(Box::new(future));
                state.new_feed_url = "".to_string();
                (state, task)
            }
            Action::Reload => {
                {
                    let feed_list = state.feed_map.values().cloned();
                    for feed in feed_list {
                        let future = fetch::get(&feed.url)
                            .map(move |body| {
                                Action::Fetched(feed.url.to_owned(), body.as_string().unwrap())
                            })
                            .map_err(|_| ());
                        task.push(Box::new(future));
                    }
                }
                (state, task)
            }

            Action::Fetched(feed_url, resp) => {
                if let Ok(atom) = AtomFeed::from_str(&resp) {
                    let feed = Feed::from_atom(feed_url.clone(), &atom);
                    state.feed_map.insert(feed_url.clone(), feed);

                    return (state, task);
                }

                let rss = Channel::from_str(&resp).unwrap();
                let feed = Feed::from_rss(feed_url.clone(), &rss);
                state.feed_map.insert(feed_url.clone(), feed);
                (state, task)
            }
        }
    }

    fn view(&self, state: State) -> View<Action> {
        view! {
            <div>
                <h1>wino</h1>
                <section>
                    <input
                        value={ state.new_feed_url.clone() }
                        oninput={ |v| match v {
                            HandlerArg::String(v) => Some(Action::UpdateNewFeedUrl(v)),
                            _ => None,
                        } }
                        onkeydown={ |v| match v {
                            HandlerArg::String(ref v) if v.as_str() == "Enter" => {
                                Some(Action::AddFeed)
                            }
                            _ => None,
                        } }
                    />
                    <button onclick={ |_| Some(Action::AddFeed) }>button</button>
                </section>
                <section>
                    <button onclick={ |_| Some(Action::Reload) }>reload</button>
                </section>
                <section>
                    <h2>Feeds</h2>
                    <ul>
                    {
                        Child::from_iter(
                            state.feed_map.values().map(|feed| {
                                view! { <li>{ feed.title.clone() }</li> }
                            })
                        )
                    }
                    </ul>
                </section>
                <section>
                    <h2>Articles</h2>
                    <ul>
                    {
                        let iter = state.feed_map
                            .values()
                            .flat_map(|feed| {
                                feed.article_map
                                    .values()
                                    .map(move |article| (feed.title.clone(), article))
                            });
                        let mut article_vec = Vec::from_iter(iter);
                        article_vec.sort_by(|(_, a), (_, b)| b.date.cmp(&a.date));
                        Child::from_iter(
                            article_vec.iter().map(|(feed_title, article)| {
                                view! { <li>{ feed_title.clone() }: <a target="_blank" href={ article.url.clone() }>{ article.title.clone() }</a></li> }
                            })
                        )
                    }
                    </ul>
                </section>
            </div>
        }
    }
}

impl Default for WinoApp {
    fn default() -> WinoApp {
        WinoApp
    }
}

#[wasm_bindgen]
pub fn run() {
    set_panic_hook();

    WebRuntime::<WinoApp>::new(
        window()
            .unwrap()
            .document()
            .unwrap()
            .query_selector("#container")
            .unwrap()
            .unwrap(),
        State::default(),
    )
    .run();
}
