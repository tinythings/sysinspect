use modlib;
use std::io::Error;

fn main() -> Result<(), Error> {
    let x = modlib::runtime::get_call_args()?;
    println!("Args: {:?}", x.args());
    println!("Options: {:?}", x.options());
    println!("Payload: {:?}", x.ext());
    println!("Timeout: {:?}", x.timeout());
    println!("Quiet: {:?}", x.quiet());

    println!("---");
    let r = modlib::runtime::PluginResponse::new("Something".to_string());
    modlib::runtime::send_call_response(&r)?;

    Ok(())
}
