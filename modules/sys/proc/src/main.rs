use libsysinspect::{self, runtime};
use std::io::Error;

fn main() -> Result<(), Error> {
    let x = runtime::get_call_args()?;
    println!("Args: {:?}", x.args());
    println!("Options: {:?}", x.options());
    println!("Payload: {:?}", x.ext());
    println!("Timeout: {:?}", x.timeout());
    println!("Quiet: {:?}", x.quiet());

    println!("---");
    let r = runtime::PluginResponse::new("Something".to_string());
    runtime::send_call_response(&r)?;

    Ok(())
}
