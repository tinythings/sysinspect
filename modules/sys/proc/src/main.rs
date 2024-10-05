use libsysinspect::modlib::modinit::ModDoc;

fn main() {
    let mod_doc = libsysinspect::init_mod_doc!(ModDoc);
    mod_doc.print_help();

    /*
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
    */
}
