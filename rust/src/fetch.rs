use futures::Future;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

pub(crate) fn fetch(request: Request) -> impl Future<Item = JsValue, Error = JsValue> {
    let window = web_sys::window().unwrap();
    let request_promise = window.fetch_with_request(&request);

    JsFuture::from(request_promise)
        .and_then(|resp_value| {
            let resp: Response = resp_value.dyn_into().unwrap();
            resp.text()
        })
        .and_then(JsFuture::from)
}

pub(crate) fn get(url: &str) -> impl Future<Item = JsValue, Error = JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts).unwrap();

    fetch(request)
}
