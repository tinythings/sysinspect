use crate::{MasterInterfaceType, api::v1::TAG_MODELS};
use actix_web::{
    HttpResponse, Result, get,
    web::{Data, Json, Query},
};
use libsysinspect::{
    cfg::mmconf::MinionConfig,
    mdescr::{mspec, mspecdef::ModelSpec},
    util::dataconv,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelInfo {
    /// The name of the model
    name: String,

    /// A brief description of the model
    description: String,

    /// The version of the model
    version: String,

    /// The author of the model
    maintainer: String,

    /// Entity to a vector of bound actions
    #[serde(rename = "entity-actions")]
    entities: HashMap<String, Vec<String>>,

    /// The number of defined events in the model
    events: i64,
}

impl ModelInfo {
    pub fn from(mdl: ModelSpec) -> Self {
        let mut nfo = ModelInfo {
            name: mdl.name().to_string(),
            description: mdl.description().to_string(),
            version: mdl.version().to_string(),
            maintainer: mdl.maintainer().to_string(),
            entities: HashMap::new(),
            events: 0,
        };

        let entities = mdl.top("entities");
        if let Some(v) = entities
            && let Some(map) = v.as_mapping()
        {
            for entity in map.keys().map(|k| dataconv::as_str(Some(k.clone()))).collect::<Vec<_>>() {
                nfo.add_entity(entity, vec![]); // XXX: Add bound actions later
            }
        }

        nfo
    }

    fn add_entity(&mut self, name: String, actions: Vec<String>) {
        self.entities.insert(name, actions);
    }
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ModelResponse {
    models: Vec<ModelInfo>,
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
    responses(
        (status = 200, description = "List of available models", body = [ModelNameResponse])
    )
)]
#[allow(unused)]
#[get("/api/v1/model/names")]
pub async fn model_names_handler(master: Data<MasterInterfaceType>) -> Json<ModelNameResponse> {
    let mut master = master.lock().await;
    Json(ModelNameResponse { models: master.cfg().await.fileserver_models().to_owned() })
}
#[utoipa::path(
    get,
    path = "/api/v1/model/descr",
    tag = TAG_MODELS,
    operation_id = "getModelDetails",
    description = "Retrieves detailed information about a specific model in the SysInspect system. The model includes its name, description, version, maintainer, and statistics about its entities, actions, constraints, and events.",
    params(
        ("name" = String, Query, description = "Name of the model to retrieve details for")
    ),
    responses(
        (status = 200, description = "Detailed information about the model", body = ModelResponse)
    )
)]
#[allow(unused)]
#[get("/api/v1/model/descr")]
pub async fn model_descr_handler(master: Data<MasterInterfaceType>, query: Query<HashMap<String, String>>) -> Result<HttpResponse> {
    let name = query.get("name").cloned().unwrap_or_default();
    if name.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ModelResponseError { error: "Missing 'name' query parameter".to_string() }));
    }

    let mut master = master.lock().await;
    let cfg = master.cfg().await.clone();
    let models = cfg.fileserver_models();
    if !models.contains(&name) {
        return Ok(HttpResponse::NotFound().json(ModelResponseError { error: format!("Model '{}' not found", name) }));
    }

    let root = cfg.fileserver_mdl_root(false);

    match mspec::load(Arc::new(MinionConfig::default()), &format!("{}/{}", root.to_str().unwrap_or_default(), name), None, None) {
        Err(e) => {
            log::error!("Failed to load model '{}': {}", name, e);
            Ok(HttpResponse::InternalServerError().json(ModelResponseError { error: format!("Failed to load model '{}': {}", name, e) }))
        }
        Ok(mdl) => {
            let info = ModelInfo::from(mdl);
            Ok(HttpResponse::Ok().json(ModelResponse { models: vec![info] }))
        }
    }
}
