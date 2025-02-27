use actix_web::{rt::System, web, App, HttpResponse, HttpServer, Responder};
use libsysinspect::{
    cfg::mmconf::{MasterConfig, CFG_FILESERVER_ROOT, DEFAULT_SYSINSPECT_ROOT},
    SysinspectError,
};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
};

// Separate handler on every HTTP call
async fn serve_file(path: web::Path<PathBuf>, _cfg: web::Data<MasterConfig>) -> impl Responder {
    let pth = Path::new(DEFAULT_SYSINSPECT_ROOT).join(CFG_FILESERVER_ROOT).join(path.into_inner());
    log::debug!("Requested local file: {:?}", pth);
    if pth.is_file() {
        return HttpResponse::Ok().body(fs::read(pth).unwrap());
    }
    log::error!("File {:?} was not found", pth);
    HttpResponse::NotFound().body("File not found")
}

/// Start fileserver
pub async fn start(cfg: MasterConfig) -> Result<(), SysinspectError> {
    thread::spawn(move || {
        let c_cfg = cfg.clone();
        System::new().block_on(async move {
            let server = HttpServer::new(move || {
                App::new().app_data(web::Data::new(cfg.clone())).service(web::resource("/{path:.*}").to(serve_file))
            })
            .bind(c_cfg.fileserver_bind_addr());

            match server {
                Ok(server) => {
                    if let Err(err) = server.run().await {
                        Err(err)
                    } else {
                        Ok(())
                    }
                }
                Err(err) => Err(err),
            }
        })
    });

    Ok(())
}
