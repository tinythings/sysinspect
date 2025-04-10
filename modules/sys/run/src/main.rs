use core::str;
use libmodcore::init_mod_doc;
use libmodcore::{
    getenv,
    modinit::ModInterface,
    response::ModResponse,
    runtime::{self, ModRequest, get_arg, get_call_args, get_opt, send_call_response},
};

use serde_json::json;
use std::{
    io::Write,
    process::{Command, Stdio},
};

/// Call an external command.
/// In a pretty ugly way...
fn call(cmd: &str, send: &str, locale: &str, env: &str, disown: bool) -> ModResponse {
    let mut resp = runtime::new_call_response();
    resp.set_retcode(1);

    let args = cmd.split_whitespace().collect::<Vec<&str>>();
    let mut l_loc = locale;
    if locale.is_empty() {
        l_loc = "C";
    }

    let mut process = Command::new(args[0]);
    process.env_clear();
    process.args(&args[1..]);

    // Set locale
    [("LC_ALL", l_loc), ("LANG", l_loc)].iter().for_each(|(n, v)| {
        process.env(n, v);
    });

    // Set env
    getenv(env).into_iter().for_each(|(vr, vl)| {
        process.env(vr, vl);
    });

    if disown {
        match process.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(_) => {
                resp.set_retcode(0);
                resp.set_message(&format!("'{}' is running in background", cmd));
            }
            Err(err) => resp.set_message(&err.to_string()),
        }
        return resp;
    }

    match process.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(mut p) => {
            if !send.is_empty() {
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(send.as_bytes()) {
                        resp.set_message(&err.to_string());
                        return resp;
                    }
                }
            }

            // XXX: In the moment this is blocking. If a command blocks,
            // then the whole thing will wait until forever. A better approach
            // would be to take stdout and then read it in a reader, while maintaining
            // a timeout and then kill the child.
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

    call(&cmd, &get_arg(rt, "send"), &get_arg(rt, "locale"), &get_arg(rt, "env"), get_opt(rt, "disown"))
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
