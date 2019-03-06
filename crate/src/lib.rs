#![feature(proc_macro_hygiene)]

extern crate atom_syndication;
extern crate chrono;
extern crate console_error_panic_hook;
extern crate futures;
extern crate js_sys;
extern crate rss;
extern crate serde;
extern crate serde_json;
extern crate squark;
extern crate squark_macros;
extern crate squark_web;
extern crate wasm_bindgen;
extern crate wasm_bindgen_futures;
extern crate web_sys;

use atom_syndication::Feed as AtomFeed;
use console_error_panic_hook::set_once as set_panic_hook;
use futures::Future;
use js_sys::Promise;
use js_sys::Function;
use rss::Channel;
use squark::{App, Child, HandlerArg, Runtime, Task, View};
use squark_macros::view;
use squark_web::WebRuntime;
use std::iter::FromIterator;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, window, VisibilityState};
use serde_json::json;

mod fetch;
mod state;

use state::{Feed, State};

const STATE_KEY: &str = "state";
const AUTO_RELOAD_MINUTES: i32 = 5;

const DEFAULT_TITLE: &str = "wino";
const HIGHLIGHT_TITLE: &str = "(*)wino";


#[wasm_bindgen]
extern "C" {
    type Chrome;

    static chrome: Chrome;
    #[wasm_bindgen(method, getter)]
    fn permissions(this: &Chrome) -> Permissions;

    type Permissions;
    #[wasm_bindgen(method)]
    fn request(this: &Permissions, arg: &JsValue, cb: &Function);
    #[wasm_bindgen(method)]
    fn remove(this: &Permissions, arg: &JsValue);
}

#[derive(Clone, Debug)]
enum Action {
    Empty,
    Fetch(String),
    UpdateNewFeedUrl(String),
    RemoveFeed(String),
    ToggleFeedVisible(String),
    AddFeed,
    Fetched(String, String),
    Reload,
    AutoReload,
}

#[derive(Clone, Debug)]
struct WinoApp;

impl WinoApp {
    fn _reducer(&self, mut state: State, action: Action) -> (State, Task<Action>) {
        let mut task = Task::empty();
        match action {
            Action::Empty => (state, task),
            Action::UpdateNewFeedUrl(url) => {
                state.new_feed_url = url;
                (state, task)
            }
            Action::AddFeed => {
                let new_feed_url = state.new_feed_url.clone();
                let future = request_permission(&new_feed_url).map(|b| {
                    if b {
                        return Action::Fetch(new_feed_url);
                    }
                    Action::Empty
                });
                task.push(Box::new(future));
                (state, task)
            }
            Action::Fetch(url) => {
                let future = fetch::get(&url)
                    .map(move |body| Action::Fetched(url, body.as_string().unwrap()))
                    .map_err(|_| ());
                task.push(Box::new(future));
                state.new_feed_url = "".to_string();
                (state, task)
            }
            Action::AutoReload => {
                task.push(Box::new(timeout(
                    Action::AutoReload,
                    1000 * 60 * AUTO_RELOAD_MINUTES,
                )));
                task.push(Box::new(timeout(Action::Reload, 0)));
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
            Action::RemoveFeed(url) => {
                state.feed_map.remove(&url);
                remove_permission(&url);

                (state, task)
            }
            Action::ToggleFeedVisible(url) => {
                state.feed_map
                    .entry(url)
                    .and_modify(|f| { f.visible = !f.visible });

                (state, task)
            }
        }
    }
}

impl App for WinoApp {
    type State = State;
    type Action = Action;

    fn reducer(&self, state: State, action: Action) -> (State, Task<Action>) {
        let old_state = state.clone();

        let (state, task) = self._reducer(state, action);

        if state != old_state {
            let window = window().unwrap();
            let document = window.document().unwrap();

            if document.visibility_state() == VisibilityState::Hidden {
                document.set_title(HIGHLIGHT_TITLE);
            }

            let storage = window.local_storage().unwrap().unwrap();
            storage
                .set_item(STATE_KEY, &serde_json::to_string(&state).unwrap())
                .unwrap();
        }

        (state, task)
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
                            state.feed_map.clone().into_iter().enumerate().map(|(i, (key, feed))| {
                                let key_1 = key.clone();
                                let id = format!("feed-{}-toggle", i);
                                view! {
                                    <li>
                                        <input
                                            type="checkbox"
                                            onclick={ move |_| Some(Action::ToggleFeedVisible(key.to_owned())) }
                                            checked={feed.visible}
                                            id={ id.clone() }
                                        />
                                        <label for={ id }>{ feed.title.clone() }</id>
                                        <button onclick={ move |_| Some(Action::RemoveFeed(key_1.to_owned())) }>x</button>
                                    </li>
                                }
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
                            .filter(|feed| feed.visible)
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

fn on_visibility_change() {
    let window = window().unwrap();
    let document = window.document().unwrap();

    if document.visibility_state() == VisibilityState::Visible {
        document.set_title(DEFAULT_TITLE);
    }
}

fn request_permission(url: &str) -> impl Future<Item = bool, Error = ()> {
    let arg = json!({
        "origins": [url]
    });
    let p = Promise::new(&mut move |resolve, _| {
        let closure = Closure::wrap(Box::new(move |b: bool| {
            resolve.call1(&JsValue::null(), &b.into()).unwrap();
        }) as Box<FnMut(_)>);
        chrome.permissions().request(&JsValue::from_serde(&arg).unwrap(), closure.as_ref().unchecked_ref());
        closure.forget();
    });
    JsFuture::from(p)
        .map(|b| b.as_bool().unwrap())
        .map_err(|e| panic!("delay errored; err={:?}", e))
}

fn remove_permission(url: &str) {
    let arg = json!({
        "origins": [url]
    });
    chrome.permissions().remove(&JsValue::from_serde(&arg).unwrap());
}

fn timeout<T>(v: T, msec: i32) -> impl Future<Item = T, Error = ()> {
    let p = Promise::new(&mut move |resolve, _| {
        let closure = Closure::wrap(Box::new(move |_: JsValue| {
            resolve.call0(&JsValue::null()).unwrap();
        }) as Box<FnMut(_)>);
        window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                msec,
            )
            .unwrap();
        closure.forget();
    });
    JsFuture::from(p)
        .map(move |_| v)
        .map_err(|e| panic!("delay errored; err={:?}", e))
}

#[wasm_bindgen]
pub fn run() {
    set_panic_hook();

    let window = window().unwrap();
    let document = window.document().unwrap();

    let storage = window.local_storage().unwrap().unwrap();

    let state = storage
        .get_item(STATE_KEY)
        .unwrap()
        .map(|s| serde_json::from_str(&s).unwrap())
        .unwrap_or_default();

    let mut task = Task::empty();
    task.push(Box::new(timeout(
        Action::AutoReload,
        1000 * 60 * AUTO_RELOAD_MINUTES,
    )));

    let closure = Closure::wrap(Box::new(on_visibility_change) as Box<Fn()>);
    document.set_onvisibilitychange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    WebRuntime::<WinoApp>::new(
        document.query_selector("#container").unwrap().unwrap(),
        state,
    )
    .run_with_task(task);
}
