use actix_web::{App, HttpResponse, HttpServer, Responder, rt::System, web};
use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{CFG_FILESERVER_ROOT, DEFAULT_SYSINSPECT_ROOT, MasterConfig},
};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
};

// Separate handler on every HTTP call
async fn serve_file(path: web::Path<PathBuf>, _cfg: web::Data<MasterConfig>) -> impl Responder {
    let pth = Path::new(DEFAULT_SYSINSPECT_ROOT).join(CFG_FILESERVER_ROOT).join(path.into_inner());
    log::debug!("Requested local file: {pth:?}");
    if pth.is_file() {
        return HttpResponse::Ok().body(fs::read(pth).unwrap());
    }
    log::error!("File {pth:?} was not found");
    HttpResponse::NotFound().body("File not found")
}

/// Start fileserver
pub async fn start(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let cfg_clone = cfg.clone();
    thread::spawn(move || {
        let c_cfg = cfg_clone.clone();
        System::new().block_on(async move {
            let server =
                HttpServer::new(move || App::new().app_data(web::Data::new(cfg_clone.clone())).service(web::resource("/{path:.*}").to(serve_file)))
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
    log::info!("Fileserver started at address {}", cfg.fileserver_bind_addr());
    Ok(())
}
