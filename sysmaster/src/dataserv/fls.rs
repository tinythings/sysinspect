use actix_web::{App, HttpResponse, HttpServer, Responder, rt::System, web};
use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::{
    CFG_FILESERVER_ROOT, CFG_MODELS_ROOT, CFG_MODREPO_ROOT, CFG_SENSORS_ROOT, CFG_TRAIT_FUNCTIONS_ROOT, CFG_TRAITS_ROOT, MasterConfig,
};
use std::{fs, path::PathBuf, thread};

/// Initialize the file server environment by creating necessary directories if they do not exist.
fn init_fs_env(cfg: &MasterConfig) -> Result<(), SysinspectError> {
    let root = cfg.root_dir().join(CFG_FILESERVER_ROOT);
    if !root.exists() {
        log::info!("Created file server root directory at {}", root.display().to_string().bright_yellow());
        fs::create_dir_all(&root)?;
    }

    for sub in [CFG_TRAIT_FUNCTIONS_ROOT, CFG_MODELS_ROOT, CFG_MODREPO_ROOT, CFG_TRAITS_ROOT, CFG_SENSORS_ROOT] {
        let subdir = root.join(sub);
        if !subdir.exists() {
            log::info!("Created file server subdirectory at {}", subdir.display().to_string().bright_yellow());
            fs::create_dir_all(&subdir)?;
        }
    }

    Ok(())
}

// Separate handler on every HTTP call
async fn serve_file(path: web::Path<PathBuf>, cfg: web::Data<MasterConfig>) -> impl Responder {
    let pth = cfg.root_dir().join(CFG_FILESERVER_ROOT).join(path.into_inner());
    log::debug!("Requested local file: {}", pth.display().to_string().bright_yellow());
    if pth.is_file() {
        return HttpResponse::Ok().body(fs::read(pth).unwrap());
    }
    log::error!("File {} was not found", pth.display().to_string().bright_red());
    HttpResponse::NotFound().body("File not found")
}

/// Start fileserver
pub async fn start(cfg: MasterConfig) -> Result<(), SysinspectError> {
    log::info!("Starting file server");

    let cfg_clone = cfg.clone();
    init_fs_env(&cfg)?;

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
