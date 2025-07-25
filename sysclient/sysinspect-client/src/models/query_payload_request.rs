/*
 * SysInspect API
 *
 * SysInspect Web API for interacting with the master interface.
 *
 * The version of the OpenAPI document: 0.1.0
 * 
 * Generated by: https://openapi-generator.tech
 */

use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryPayloadRequest {
    #[serde(rename = "context")]
    pub context: std::collections::HashMap<String, String>,
    #[serde(rename = "mid")]
    pub mid: String,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "query")]
    pub query: String,
    #[serde(rename = "traits")]
    pub traits: String,
}

impl QueryPayloadRequest {
    pub fn new(context: std::collections::HashMap<String, String>, mid: String, model: String, query: String, traits: String) -> QueryPayloadRequest {
        QueryPayloadRequest {
            context,
            mid,
            model,
            query,
            traits,
        }
    }
}

