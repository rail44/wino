#![feature(proc_macro_hygiene)]

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern crate atom_syndication;
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
extern crate wee_alloc;

use atom_syndication::Feed as AtomFeed;
use console_error_panic_hook::set_once as set_panic_hook;
use futures::Future;
use squark::{App, Child, Runtime, Task, View};
use squark_macros::view;
use squark_web::WebRuntime;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use web_sys::{console, window};

mod fetch;

#[derive(Clone, Debug)]
enum Action {
    Fetched(JsValue),
    Click,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Feed {
    title: String,
    link: String,
    url: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Article {
    title: String,
    date: String,
    url: String,
}

#[derive(Clone, Debug, PartialEq)]
struct State {
    article_map: HashMap<String, Article>,
    feed_list: Vec<Feed>,
}

impl Default for State {
    fn default() -> State {
        State {
            article_map: HashMap::new(),
            feed_list: vec![
                Feed {
                    title: "GIGAZINE".to_owned(),
                    link: "".to_owned(),
                    url: "https://gigazine.net/news/rss_atom/".to_owned(),
                },
                Feed {
                    title: "ギズモード・ジャパン".to_owned(),
                    link: "".to_owned(),
                    url: "https://rustwasm.github.io/feed.xml".to_owned(),
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
struct WinoApp;
impl App for WinoApp {
    type State = State;
    type Action = Action;

    fn reducer(&self, mut state: State, action: Action) -> (State, Task<Action>) {
        let mut task = Task::empty();
        match action {
            Action::Click => {
                for feed in state.feed_list.iter() {
                    let future = fetch::get(&feed.url)
                        .map(Action::Fetched)
                        .map_err(|_| ());
                    task.push(Box::new(future));
                }
            }
            Action::Fetched(resp) => {
                let feed = AtomFeed::from_str(&resp.as_string().unwrap()).unwrap();
                for entry in feed.entries() {
                    let date = entry.published().unwrap_or("").to_string();
                    let article = Article {
                        title: entry.title().to_string(),
                        url: entry
                            .links()
                            .get(0)
                            .map_or("", |link| link.href())
                            .to_string(),
                        date,
                    };
                    let id = entry.id();
                    state.article_map.insert(id.to_string(), article);
                }
                console::log_1(&feed.title().into());
            }
        };
        (state, task)
    }

    fn view(&self, state: State) -> View<Action> {
        view! {
            <div>
                <h1>wino</h1>
                <button onclick={ |_| Some(Action::Click) }>button</button>
                <ul>
                {
                    let mut article_vec = Vec::from_iter(state.article_map.values());
                    article_vec.sort_by(|a, b| b.date.cmp(&a.date));
                    Child::from_iter(
                        article_vec.iter().map(|feed| {
                            view! { <li><a target="_blank" href={ feed.url.clone() }>{ feed.title.clone() }</a></li> }
                        })
                    )
                }
                </ul>
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
