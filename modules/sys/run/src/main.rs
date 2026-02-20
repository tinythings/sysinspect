use core::str;
use libmodcore::init_mod_doc;
use libmodcore::runtime::get_arg_default;
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

struct ModParams {
    send: String,
    locale: String,
    env: String,
    disown: bool,
    strip: bool,
}

/// Call an external command.
/// In a pretty ugly way...
fn call(cmd: &str, params: ModParams) -> ModResponse {
    let mut resp = runtime::new_call_response();
    resp.set_retcode(1);
    resp.set_message("N/A");

    let argv = shlex::split(cmd).unwrap_or_default();
    if argv.is_empty() {
        resp.set_message("Missing/invalid command");
        return resp;
    }

    let mut l_loc = params.locale.as_str();
    if params.locale.is_empty() {
        l_loc = "C";
    }

    let mut process = Command::new(&argv[0]);
    process.env_clear();
    process.args(&argv[1..]);

    // Set locale
    [("LC_ALL", l_loc), ("LANG", l_loc)].iter().for_each(|(n, v)| {
        process.env(n, v);
    });

    // Set env
    getenv(&params.env).into_iter().for_each(|(vr, vl)| {
        process.env(vr, vl);
    });

    if params.disown {
        match process.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(_) => {
                resp.set_retcode(0);
                resp.set_message(&format!("'{cmd}' is running in background"));
            }
            Err(err) => resp.set_message(&err.to_string()),
        }
        return resp;
    }

    match process.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(mut p) => {
            if !params.send.is_empty()
                && let Some(mut stdin) = p.stdin.take()
                && let Err(err) = stdin.write_all(params.send.as_bytes())
            {
                resp.set_message(&err.to_string());
                return resp;
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
                                if let Err(err) = resp.set_data(json!({"stdout": if params.strip { stdout.trim() } else { stdout }})) {
                                    resp.set_message(&err.to_string());
                                } else {
                                    resp.set_retcode(0);
                                    resp.set_message("module sys.run finished");
                                }
                                resp
                            }
                            Err(err) => {
                                resp.set_message(&format!("Error getting output: {err:?}"));
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
            resp.set_message(&format!("Error running '{cmd}': {err}"));
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

    let params = ModParams {
        send: get_arg(rt, "send"),
        locale: get_arg(rt, "locale"),
        env: get_arg(rt, "env"),
        disown: get_opt(rt, "disown"),
        strip: get_arg_default(rt, "strip", "true").to_lowercase().eq("true"),
    };
    call(&cmd, params)
}

fn main() {
    let mod_doc = init_mod_doc!(ModInterface);
    if mod_doc.print_help() {
        return;
    }

    match get_call_args() {
        Ok(rt) => match send_call_response(&run_mod(&rt)) {
            Ok(_) => {}
            Err(err) => println!("Module runtime error: {err}"),
        },
        Err(err) => {
            println!("Arguments error: {err}")
        }
    }
}
