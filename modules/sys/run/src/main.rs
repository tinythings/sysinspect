use core::str;
use libsysinspect::{
    init_mod_doc,
    modlib::{
        modinit::ModInterface,
        response::ModResponse,
        runtime::{self, get_arg, get_call_args, send_call_response, ModRequest},
    },
};
use serde_json::json;
use std::{
    io::Write,
    process::{Command, Stdio},
};

/// Call an external command.
/// In a pretty ugly way...
fn call(cmd: &str, send: &str, disown: bool) -> ModResponse {
    let mut resp = runtime::new_call_response();
    resp.set_retcode(1);

    let args = cmd.split_whitespace().collect::<Vec<&str>>();

    match Command::new(args[0]).args(&args[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(mut p) => {
            if !send.is_empty() {
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(send.as_bytes()) {
                        resp.set_message(&err.to_string());
                        return resp;
                    }
                }
            }

            if disown {
                resp.set_message("Disown is not yet implemented");
                resp
            } else {
                match p.wait_with_output() {
                    Ok(out) => {
                        if out.status.success() {
                            match str::from_utf8(&out.stdout) {
                                Ok(stdout) => {
                                    if let Err(err) = resp.set_data(json!({"stdout": stdout})) {
                                        resp.set_message(&err.to_string());
                                    } else {
                                        resp.set_retcode(0);
                                        resp.set_message(&format!("\"{}\" finished", cmd));
                                    }
                                    resp
                                }
                                Err(err) => {
                                    resp.set_message(&format!("Error getting output: {:?}", err));
                                    resp
                                }
                            }
                        } else {
                            match str::from_utf8(&out.stderr) {
                                Ok(stderr) => {
                                    let mut r = runtime::new_call_response();
                                    r.set_retcode(out.status.code().unwrap_or(1));
                                    r.set_message(stderr);
                                }
                                Err(err) => {
                                    resp.set_message(&err.to_string());
                                }
                            }
                            resp
                        }
                    }
                    Err(err) => {
                        resp.set_message(&err.to_string());
                        resp
                    }
                }
            }
        }
        Err(err) => {
            resp.set_message(&format!("Error running '{}': {}", cmd, err));
            resp
        }
    }
}

fn run_mod(rt: &ModRequest) -> ModResponse {
    let mut res = ModResponse::new();

    let cmd = get_arg(rt, "cmd");
    if cmd.is_empty() {
        res.set_retcode(1);
        res.set_message("Missing command");
        return res;
    }

    call(&cmd, &get_arg(rt, "send"), false)
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match get_call_args() {
        Ok(rt) => match send_call_response(&run_mod(&rt)) {
            Ok(_) => {}
            Err(err) => println!("Runtime error: {}", err),
        },
        Err(err) => {
            println!("Arguments error: {}", err)
        }
    }
}
