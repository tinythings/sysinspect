use libsysinspect::{self, runtime};
use std::{collections::HashMap, io::Error};

fn main() {
    /*
    let x = runtime::get_call_args()?;
    println!("Args: {:?}", x.args());
    println!("Options: {:?}", x.options());
    println!("Payload: {:?}", x.ext());
    println!("Timeout: {:?}", x.timeout());
    println!("Quiet: {:?}", x.quiet());

    println!("---");

    let mut v = HashMap::<String, String>::new();
    v.insert("something".to_string(), "world".to_string());

    let r =
        runtime::PluginResponse::new(libsysinspect::tpl::interpolate("Hello, $(something)?!", &v));
    runtime::send_call_response(&r)?;

    println!(
        "{:?}",
        libsysinspect::tpl::extract(
            "here $(is.a.test) of stuff $(that) could matter $(at.some) point"
        )
    );
    */

    let r = match libsysinspect::mdl::mspec::load(".") {
        Ok(m) => {
            println!("{:?}", m);
        }
        Err(err) => println!("Error: {}", err),
    };
}
