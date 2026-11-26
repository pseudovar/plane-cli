use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: Uuid,
    pub name: String,
    pub identifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub sequence: Option<f64>,
    #[serde(default, rename = "default")]
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelEntry {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

pub fn projects_from_value(v: &Value) -> Result<Vec<ProjectEntry>, CoreError> {
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| CoreError::Cache("projects response missing results array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::Cache("project entry missing id".into()))?;
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let identifier = item
            .get("identifier")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        out.push(ProjectEntry {
            id,
            name,
            identifier,
        });
    }
    Ok(out)
}

pub fn states_from_value(v: &Value) -> Result<Vec<StateEntry>, CoreError> {
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| CoreError::Cache("states response missing results array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::Cache("state entry missing id".into()))?;
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let group = item.get("group").and_then(|x| x.as_str()).map(String::from);
        let color = item.get("color").and_then(|x| x.as_str()).map(String::from);
        let sequence = item.get("sequence").and_then(|x| x.as_f64());
        let is_default = item.get("default").and_then(|x| x.as_bool());
        out.push(StateEntry {
            id,
            name,
            group,
            color,
            sequence,
            is_default,
        });
    }
    Ok(out)
}

pub fn labels_from_value(v: &Value) -> Result<Vec<LabelEntry>, CoreError> {
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| CoreError::Cache("labels response missing results array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::Cache("label entry missing id".into()))?;
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let color = item.get("color").and_then(|x| x.as_str()).map(String::from);
        let description = item
            .get("description")
            .and_then(|x| x.as_str())
            .map(String::from);
        let sort_order = item.get("sort_order").and_then(|x| x.as_f64());
        out.push(LabelEntry {
            id,
            name,
            color,
            description,
            sort_order,
        });
    }
    Ok(out)
}
