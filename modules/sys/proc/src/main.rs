use libsysinspect::modlib::{runtime, tpl};
use std::collections::HashMap;

fn main() {
    let x = runtime::get_call_args().unwrap();
    println!("Args: {:?}", x.args());
    println!("Options: {:?}", x.options());
    println!("Payload: {:?}", x.ext());
    println!("Timeout: {:?}", x.timeout());
    println!("Quiet: {:?}", x.quiet());

    println!("---");

    let mut v = HashMap::<String, String>::new();
    v.insert("something".to_string(), "world".to_string());

    let r = runtime::PluginResponse::new(tpl::interpolate("Hello, $(something)?!", &v));
    runtime::send_call_response(&r);

    println!("{:?}", tpl::extract("here $(is.a.test) of stuff $(that) could matter $(at.some) point"));
}
