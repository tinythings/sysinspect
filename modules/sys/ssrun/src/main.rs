use anyhow::Context;
use dotenv::dotenv;
use libsysinspect::{
    init_mod_doc,
    modlib::{
        getenv,
        modinit::ModInterface,
        response::ModResponse,
        runtime::{get_arg, get_call_args, send_call_response, ModRequest},
    },
};
use maplit::hashmap;
use serde_json::json;
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::PathBuf;

struct ModArgs {
    user: String,
    host: String,
    port: usize,
    rsa_prk: Option<PathBuf>,
    password: Option<String>,
    cmd: String,
    locale: String,
    env: String,
}

/// Call SSH module
fn call(mrg: ModArgs) -> ModResponse {
    let mut resp = ModResponse::default();
    resp.set_retcode(1); // Failure by default

    dotenv().ok();

    let addr = format!("{}:{}", mrg.host, mrg.port);
    let stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(err) => {
            resp.set_message(&format!("Unable to connect to: {addr}: {err}"));
            return resp;
        }
    };

    let mut sess = match Session::new() {
        Ok(s) => s,
        Err(err) => {
            resp.set_message(&format!("Unable to create SSH session: {err}"));
            return resp;
        }
    };

    sess.set_tcp_stream(stream);

    if let Err(err) = sess.handshake() {
        resp.set_message(&format!("Failed to initialise SSH handshake: {err}"));
        return resp;
    }

    if mrg.rsa_prk.is_none() {
        if let Some(password) = mrg.password {
            // Mad Idea ™, but the user still wants that... ¯\_(ツ)_/¯
            if let Err(err) = sess.userauth_password(&mrg.user, &password).context("SSH password authentication failed") {
                resp.set_message(&format!("Authentication error: {err}"));
                return resp;
            }
        } else {
            resp.set_message("RSA key or a password must be supplied");
            return resp;
        }
    } else if let Err(err) =
        sess.userauth_pubkey_file(&mrg.user, None, &mrg.rsa_prk.unwrap().as_path(), mrg.password.as_deref()).with_context(|| {
            if mrg.password.is_some() {
                "SSH key authentication failed: Incorrect passphrase or key."
            } else {
                "SSH key authentication failed: Incorrect key or permissions."
            }
        })
    {
        resp.set_message(&format!("Authentication error: {err}"));
        return resp;
    }

    if !sess.authenticated() {
        resp.set_message("Authentication failed");
        return resp;
    }

    // Channel
    let mut channel = match sess.channel_session() {
        Ok(c) => c,
        Err(err) => {
            resp.set_message(&format!("SSH channel error: {err}"));
            return resp;
        }
    };

    // Set locale
    [("LC_ALL", &mrg.locale), ("LANG", &mrg.locale)].iter().for_each(|(k, v)| {
        channel.setenv(k, v).unwrap_or_default();
    });

    // Set env
    getenv(&mrg.env).into_iter().for_each(|(k, v)| {
        channel.setenv(&k, &v).unwrap_or_default();
        println!("Added {k} -> {v}");
    });

    if let Err(err) = channel.exec(&mrg.cmd).with_context(|| format!("Failed to execute command: {}", mrg.cmd)) {
        resp.set_message(&err.to_string());
        return resp;
    }

    // Read the command output
    let mut stout = String::new();
    if let Err(err) = channel.read_to_string(&mut stout).context("Failed to read command output") {
        resp.set_message(&err.to_string());
        return resp;
    };

    if let Err(err) = resp.set_data(json!(hashmap! {"stdout" => stout.trim().to_owned(), "cmd" => mrg.cmd.to_owned()})) {
        resp.set_message(&format!("Unable to add output: {err}"));
        return resp;
    }

    // Check the exit status
    let errcode = match channel.exit_status().context("Failed to get exit status") {
        Ok(errcode) => errcode,
        Err(err) => {
            resp.set_message(&err.to_string());
            return resp;
        }
    };

    if let Err(err) = channel.close().context("Failed to close channel") {
        resp.set_message(&err.to_string());
        return resp;
    }

    if let Err(err) = channel.wait_close().context("Failed to wait for channel to close") {
        resp.set_message(&err.to_string());
        return resp;
    }

    resp.set_message(&format!("\"{}\" finished", mrg.cmd));
    resp.set_retcode(errcode);
    resp
}

fn run_mod(rt: &ModRequest) -> ModResponse {
    let mut res = ModResponse::new();

    let cmd = get_arg(rt, "cmd");
    if cmd.is_empty() {
        res.set_retcode(1);
        res.set_message("Missing command");
        return res;
    }

    let ma = ModArgs {
        user: get_arg(rt, "user"),
        host: get_arg(rt, "host"),
        port: get_arg(rt, "port").parse::<usize>().unwrap_or(22),
        rsa_prk: Some(get_arg(rt, "rsakey")).filter(|p| !p.is_empty()).map(PathBuf::from),
        password: Some(get_arg(rt, "user")).filter(|p| !p.is_empty()),
        cmd: get_arg(rt, "cmd"),
        locale: get_arg(rt, "locale"),
        env: get_arg(rt, "env"),
    };

    call(ma)
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
