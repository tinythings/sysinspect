use crate::{
    MasterInterfaceType,
    api::v1::{TAG_MODELS, minions::authorise_request},
};
use actix_web::{
    HttpRequest, HttpResponse, Result, get,
    web::{Data, Query},
};
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libsysinspect::{
    cfg::mmconf::MinionConfig,
    intp::inspector::SysInspector,
    mdescr::{mspec, mspecdef::ModelSpec},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelInfo {
    /// The unique identifier of the model (Id)
    id: String,

    /// The name of the model
    name: String,

    /// A brief description of the model
    description: String,

    /// The version of the model
    version: String,

    /// The author of the model
    maintainer: String,

    /// Entity to a vector of bound actions
    #[allow(clippy::type_complexity)]
    #[serde(rename = "entity-states")]
    entities: BTreeMap<String, Vec<(String, BTreeMap<String, String>)>>, // Entity -> States
}

impl ModelInfo {
    pub fn from(mid: String, mdl: ModelSpec) -> Result<Self, SysinspectError> {
        let mut nfo = ModelInfo {
            id: mid,
            name: mdl.name().to_string(),
            description: mdl.description().to_string(),
            version: mdl.version().to_string(),
            maintainer: mdl.maintainer().to_string(),
            entities: BTreeMap::new(),
        };

        let si = SysInspector::schema(mdl.clone())?;
        for e in si.entities() {
            let mut states = Vec::<(String, BTreeMap<String, String>)>::new();
            for action in si.actions_by_entities(vec![e.id().to_string()], None)?.into_iter() {
                states.extend(
                    action
                        .states(Some("*".to_string()))
                        .into_iter()
                        .map(|(state, ma)| (state, ma.context().iter().map(|(k, v)| (k.clone(), v.clone())).collect::<BTreeMap<String, String>>())),
                );
            }

            states.sort();
            states.dedup();

            nfo.entities.insert(e.id().to_string(), states);
        }

        Ok(nfo)
    }
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelResponse {
    model: ModelInfo,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelResponseError {
    error: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelNameResponse {
    models: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/model/names",
    tag = TAG_MODELS,
    operation_id = "listModels",
    description = "Lists all available models in the SysInspect system. Each model includes details such as its name, description, version, maintainer, and statistics about its entities, actions, constraints, and events.",
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "List of available models", body = ModelNameResponse),
        (status = 401, description = "Unauthorized", body = ModelResponseError)
    )
)]
#[allow(unused)]
#[get("/api/v1/model/names")]
pub async fn model_names_handler(req: HttpRequest, master: Data<MasterInterfaceType>) -> Result<HttpResponse> {
    if let Err(err) = authorise_request(&req).await {
        return Ok(HttpResponse::Unauthorized().json(ModelResponseError { error: err.to_string() }));
    }
    let mut master = master.lock().await;
    Ok(HttpResponse::Ok().json(ModelNameResponse { models: master.cfg().await.fileserver_models().to_owned() }))
}
#[utoipa::path(
    get,
    path = "/api/v1/model/descr",
    tag = TAG_MODELS,
    operation_id = "getModelDetails",
    description = "Retrieves detailed information about a specific model in the SysInspect system. The model includes its name, description, version, maintainer, and statistics about its entities, actions, constraints, and events.",
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("name" = String, Query, description = "Name of the model to retrieve details for")
    ),
    responses(
        (status = 200, description = "Detailed information about the model", body = ModelResponse),
        (status = 400, description = "Bad request", body = ModelResponseError),
        (status = 401, description = "Unauthorized", body = ModelResponseError),
        (status = 404, description = "Model not found", body = ModelResponseError),
        (status = 500, description = "Failed to load model information", body = ModelResponseError)
    )
)]
#[allow(unused)]
#[get("/api/v1/model/descr")]
pub async fn model_descr_handler(
    req: HttpRequest, master: Data<MasterInterfaceType>, query: Query<IndexMap<String, String>>,
) -> Result<HttpResponse> {
    if let Err(err) = authorise_request(&req).await {
        return Ok(HttpResponse::Unauthorized().json(ModelResponseError { error: err.to_string() }));
    }
    let mid = query.get("name").cloned().unwrap_or_default(); // Model Id
    if mid.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ModelResponseError { error: "Missing 'name' query parameter".to_string() }));
    }

    let mut master = master.lock().await;
    let cfg = master.cfg().await.clone();
    let models = cfg.fileserver_models();
    if !models.contains(&mid) {
        return Ok(HttpResponse::NotFound().json(ModelResponseError { error: format!("Model '{}' not found", mid) }));
    }

    let root = cfg.fileserver_models_root(false);

    match mspec::load(Arc::new(MinionConfig::default()), &format!("{}/{}", root.to_str().unwrap_or_default(), mid), None, None) {
        Err(e) => {
            log::error!("Failed to load model '{}': {}", mid, e);
            Ok(HttpResponse::InternalServerError().json(ModelResponseError { error: format!("Failed to load model '{}': {}", mid, e) }))
        }
        Ok(mdl) => match ModelInfo::from(mid.clone(), mdl) {
            Ok(info) => Ok(HttpResponse::Ok().json(ModelResponse { model: info })),
            Err(e) => {
                log::error!("Failed to build ModelInfo for '{}': {}", mid, e);
                Ok(HttpResponse::InternalServerError().json(ModelResponseError { error: format!("Failed to build ModelInfo for '{}': {}", mid, e) }))
            }
        },
    }
}
