use crate::cache::Cache;
use crate::catalog::{
    labels_from_value, projects_from_value, states_from_value, LabelEntry, ProjectEntry, StateEntry,
};
use crate::client::HttpClient;
use crate::config::Config;
use crate::error::CoreError;
use crate::pagination::{fetch_page, PageParams};
use serde_json::{json, Value};
use uuid::Uuid;

pub struct PlaneApi {
    pub config: Config,
    client: HttpClient,
    cache: Cache,
}

impl PlaneApi {
    pub fn new(config: Config) -> Result<Self, CoreError> {
        let client = HttpClient::new(&config)?;
        let cache = Cache::new(
            config.api_base.as_str(),
            &config.workspace_slug,
            config.acting_as.as_deref(),
            config.cache_enabled,
        )?;
        Ok(Self {
            config,
            client,
            cache,
        })
    }

    fn ws(&self, tail: &str) -> String {
        format!(
            "/api/v1/workspaces/{}/{}",
            self.config.workspace_slug,
            tail.trim_start_matches('/')
        )
    }

    fn proj(&self, project_id: &Uuid, tail: &str) -> String {
        format!(
            "/api/v1/workspaces/{}/projects/{}/{}",
            self.config.workspace_slug,
            project_id,
            tail.trim_start_matches('/')
        )
    }

    /// Full paged list GET for a project subpath (e.g. `states/`, `labels/`).
    fn fetch_project_list_page(
        &self,
        project_id: &Uuid,
        subpath: &str,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, subpath);
        let page = PageParams::all();
        let (v, _, _) = fetch_page(&self.client, "GET", &path, &[], &page)?;
        Ok(v)
    }

    /// The workspace cache (one per API base, workspace and acting member).
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub fn whoami(&self) -> Result<Value, CoreError> {
        self.client
            .expect_json("GET", "/api/v1/users/me/", &[], 200..=299)
    }

    /// Loads projects for identifier resolution (cache-first, capped multi-page fetch on miss).
    pub fn fetch_projects_catalog(
        &mut self,
        refresh: bool,
    ) -> Result<Vec<ProjectEntry>, CoreError> {
        let path = self.ws("projects/");
        if !refresh {
            if let Some(cached) = self.cache.get("projects", false)? {
                return projects_from_value(&cached);
            }
        }
        let page = PageParams::all();
        let (v, _, _) = fetch_page(&self.client, "GET", &path, &[], &page)?;
        self.save_projects_to_cache(&v)?;
        projects_from_value(&v)
    }

    pub fn save_projects_to_cache(&mut self, v: &Value) -> Result<(), CoreError> {
        self.cache.put("projects", v.clone())
    }

    pub fn list_projects_public(
        &self,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.ws("projects/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_project(&self, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.ws(&format!("projects/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_project(&self, body: &Value) -> Result<Value, CoreError> {
        let path = self.ws("projects/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_project(&self, pk: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.ws(&format!("projects/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_project(&self, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.ws(&format!("projects/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    /// `POST projects/{id}/archive/` (204). On CE 1.4.2 the project stays in
    /// `GET projects/` with `archived_at` set, but its states, labels and work items read as
    /// empty, so the cached state and label catalogs are dropped.
    pub fn archive_project(&self, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.ws(&format!("projects/{pk}/archive/"));
        let v = self
            .client
            .expect_json_post("POST", &path, &json!({}), 200..=299)?;
        self.drop_project_catalogs(pk);
        Ok(v)
    }

    /// `DELETE projects/{id}/archive/` (204). Drops the cached state and label catalogs, which
    /// may hold the empty lists read while the project was archived.
    pub fn unarchive_project(&self, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.ws(&format!("projects/{pk}/archive/"));
        self.client.expect_delete(&path, 200..=299)?;
        self.drop_project_catalogs(pk);
        Ok(())
    }

    fn drop_project_catalogs(&self, pk: &Uuid) {
        self.drop_catalog(&format!("states_{pk}"));
        self.drop_catalog(&format!("labels_{pk}"));
    }

    /// `GET projects/{id}/summary/`: work-item, cycle, module, member and other counts.
    pub fn project_summary(&self, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.ws(&format!("projects/{pk}/summary/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    /// Cached workflow states for a project (`states_{project_id}` in cache).
    pub fn fetch_states_catalog(
        &mut self,
        project_id: &Uuid,
        refresh: bool,
    ) -> Result<Vec<StateEntry>, CoreError> {
        let key = format!("states_{project_id}");
        if !refresh {
            if let Some(cached) = self.cache.get(&key, false)? {
                return states_from_value(&cached);
            }
        }
        let v = self.fetch_project_list_page(project_id, "states/")?;
        self.cache.put(&key, v.clone())?;
        states_from_value(&v)
    }

    /// Cached labels for a project (`labels_{project_id}` in cache).
    pub fn fetch_labels_catalog(
        &mut self,
        project_id: &Uuid,
        refresh: bool,
    ) -> Result<Vec<LabelEntry>, CoreError> {
        let key = format!("labels_{project_id}");
        if !refresh {
            if let Some(cached) = self.cache.get(&key, false)? {
                return labels_from_value(&cached);
            }
        }
        let v = self.fetch_project_list_page(project_id, "labels/")?;
        self.cache.put(&key, v.clone())?;
        labels_from_value(&v)
    }

    pub fn list_work_items(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "work-items/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// Work-item list with extra query parameters (e.g. `expand=state`). CE 1.4.2 has no
    /// server-side assignee or state filter on this endpoint.
    pub fn list_work_items_query(
        &self,
        project_id: &Uuid,
        query: &[(String, String)],
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "work-items/");
        fetch_page(&self.client, "GET", &path, query, page)
    }

    pub fn get_work_item(&self, project_id: &Uuid, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn get_work_item_by_composite(
        &self,
        project_identifier: &str,
        issue_sequence: u64,
    ) -> Result<Value, CoreError> {
        let path = self.ws(&format!(
            "work-items/{project_identifier}-{issue_sequence}/"
        ));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_work_item(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "work-items/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_work_item(
        &self,
        project_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_work_item(&self, project_id: &Uuid, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn search_work_items(
        &self,
        q: &str,
        project_id: Option<&Uuid>,
        limit: Option<u32>,
    ) -> Result<Value, CoreError> {
        let path = self.ws("work-items/search/");
        let mut query: Vec<(String, String)> = vec![("search".into(), q.into())];
        if let Some(pid) = project_id {
            query.push(("project_id".into(), pid.to_string()));
        }
        if let Some(l) = limit {
            query.push(("limit".into(), l.to_string()));
        }
        self.client.expect_json("GET", &path, &query, 200..=299)
    }

    pub fn list_activities(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/activities/"));
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_activity(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<Value, CoreError> {
        let path = self.proj(
            project_id,
            &format!("work-items/{issue_id}/activities/{pk}/"),
        );
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn list_comments(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/comments/"));
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_comment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/comments/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_comment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/comments/"));
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_comment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/comments/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_comment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/comments/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_links(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/links/"));
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_link(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/links/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_link(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/links/"));
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_link(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/links/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_link(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/links/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_relations(&self, project_id: &Uuid, issue_id: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/relations/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_relations(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/relations/"));
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn list_modules(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "modules/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_module(&self, project_id: &Uuid, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("modules/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_module(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "modules/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_module(
        &self,
        project_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("modules/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_module(&self, project_id: &Uuid, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("modules/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_archived_modules(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "archived-modules/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// `POST modules/{id}/archive/` (204; 400 when the module cannot be archived yet).
    pub fn archive_module(&self, project_id: &Uuid, module_id: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("modules/{module_id}/archive/"));
        self.client
            .expect_json_post("POST", &path, &json!({}), 200..=299)
    }

    pub fn unarchive_module(&self, project_id: &Uuid, module_id: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(
            project_id,
            &format!("archived-modules/{module_id}/unarchive/"),
        );
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_module_work_items(
        &self,
        project_id: &Uuid,
        module_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, &format!("modules/{module_id}/module-issues/"));
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn add_module_work_items(
        &self,
        project_id: &Uuid,
        module_id: &Uuid,
        issue_ids: &[Uuid],
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("modules/{module_id}/module-issues/"));
        let issues: Vec<String> = issue_ids.iter().map(|u| u.to_string()).collect();
        let body = json!({ "issues": issues });
        self.client
            .expect_json_post("POST", &path, &body, 200..=299)
    }

    pub fn remove_module_work_item(
        &self,
        project_id: &Uuid,
        module_id: &Uuid,
        issue_id: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(
            project_id,
            &format!("modules/{module_id}/module-issues/{issue_id}/"),
        );
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    /// `GET cycles/`; `cycle_view` filters by status (`all`, `current`, `upcoming`,
    /// `completed`, `draft`, `incomplete` on CE 1.4.2).
    pub fn list_cycles(
        &self,
        project_id: &Uuid,
        cycle_view: Option<&str>,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "cycles/");
        let mut query: Vec<(String, String)> = vec![];
        if let Some(v) = cycle_view {
            query.push(("cycle_view".into(), v.into()));
        }
        fetch_page(&self.client, "GET", &path, &query, page)
    }

    pub fn list_archived_cycles(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "archived-cycles/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    pub fn get_cycle(&self, project_id: &Uuid, pk: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("cycles/{pk}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_cycle(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "cycles/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_cycle(
        &self,
        project_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("cycles/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_cycle(&self, project_id: &Uuid, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("cycles/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn archive_cycle(&self, project_id: &Uuid, cycle_id: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("cycles/{cycle_id}/archive/"));
        self.client
            .expect_json_post("POST", &path, &json!({}), 200..=299)
    }

    pub fn unarchive_cycle(&self, project_id: &Uuid, cycle_id: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(
            project_id,
            &format!("archived-cycles/{cycle_id}/unarchive/"),
        );
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_cycle_work_items(
        &self,
        project_id: &Uuid,
        cycle_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, &format!("cycles/{cycle_id}/cycle-issues/"));
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// Adds work items to a cycle. Plane allows one cycle per work item, so an item that is
    /// already in another cycle is moved here (checked on CE 1.4.2).
    pub fn add_cycle_work_items(
        &self,
        project_id: &Uuid,
        cycle_id: &Uuid,
        issue_ids: &[Uuid],
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("cycles/{cycle_id}/cycle-issues/"));
        let issues: Vec<String> = issue_ids.iter().map(|u| u.to_string()).collect();
        let body = json!({ "issues": issues });
        self.client
            .expect_json_post("POST", &path, &body, 200..=299)
    }

    pub fn remove_cycle_work_item(
        &self,
        project_id: &Uuid,
        cycle_id: &Uuid,
        issue_id: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(
            project_id,
            &format!("cycles/{cycle_id}/cycle-issues/{issue_id}/"),
        );
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    /// Moves the unfinished work items of a completed cycle to `new_cycle_id`.
    pub fn transfer_cycle_work_items(
        &self,
        project_id: &Uuid,
        cycle_id: &Uuid,
        new_cycle_id: &Uuid,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("cycles/{cycle_id}/transfer-issues/"));
        let body = json!({ "new_cycle_id": new_cycle_id.to_string() });
        self.client
            .expect_json_post("POST", &path, &body, 200..=299)
    }

    // --- labels and states (catalog writes drop the cached catalog) ---

    /// Drop a cached catalog after a successful write. A failure to drop it must not turn
    /// the write into an error, so it is queued as a warning instead.
    fn drop_catalog(&self, key: &str) {
        if let Err(e) = self.cache.remove(key) {
            crate::warnings::push_warning(format!(
                "the change was saved, but plane-cli could not drop its cached catalog ({e}); pass --refresh on the next catalog lookup"
            ));
        }
    }

    pub fn list_labels(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "labels/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// Drops the cached labels catalog (a failure is a queued warning).
    pub fn create_label(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "labels/");
        let v = self
            .client
            .expect_json_post("POST", &path, body, 200..=299)?;
        self.drop_catalog(&format!("labels_{project_id}"));
        Ok(v)
    }

    /// Drops the cached labels catalog (a failure is a queued warning).
    pub fn update_label(
        &self,
        project_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("labels/{pk}/"));
        let v = self
            .client
            .expect_json_patch("PATCH", &path, body, 200..=299)?;
        self.drop_catalog(&format!("labels_{project_id}"));
        Ok(v)
    }

    pub fn delete_label(&self, project_id: &Uuid, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("labels/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        self.drop_catalog(&format!("labels_{project_id}"));
        Ok(())
    }

    pub fn list_states(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "states/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// Drops the cached states catalog (a failure is a queued warning).
    pub fn create_state(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "states/");
        let v = self
            .client
            .expect_json_post("POST", &path, body, 200..=299)?;
        self.drop_catalog(&format!("states_{project_id}"));
        Ok(v)
    }

    /// Drops the cached states catalog (a failure is a queued warning).
    pub fn update_state(
        &self,
        project_id: &Uuid,
        pk: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("states/{pk}/"));
        let v = self
            .client
            .expect_json_patch("PATCH", &path, body, 200..=299)?;
        self.drop_catalog(&format!("states_{project_id}"));
        Ok(v)
    }

    pub fn delete_state(&self, project_id: &Uuid, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("states/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        self.drop_catalog(&format!("states_{project_id}"));
        Ok(())
    }

    // --- intake ---

    pub fn list_intake(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "intake-issues/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// `issue_id` is the work item UUID (not the intake row id) on CE 1.4.2.
    pub fn get_intake(&self, project_id: &Uuid, issue_id: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("intake-issues/{issue_id}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    /// The first intake item makes Plane add a hidden `Triage` state (group `triage`), which
    /// `states/` does not list.
    pub fn create_intake(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "intake-issues/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    pub fn update_intake(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("intake-issues/{issue_id}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_intake(&self, project_id: &Uuid, issue_id: &Uuid) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("intake-issues/{issue_id}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    // --- members and invitations ---

    /// Workspace members (a plain array with `role` on CE 1.4.2).
    pub fn list_workspace_members(&self) -> Result<Value, CoreError> {
        let path = self.ws("members/");
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    /// Project members with `role` and `is_active` (`project-members-lite/`). The `id` of
    /// each row is the user id. Removed members stay listed with `is_active: false`.
    pub fn list_project_members(
        &self,
        project_id: &Uuid,
        page: &PageParams,
    ) -> Result<(Value, Option<String>, bool), CoreError> {
        let path = self.proj(project_id, "project-members-lite/");
        fetch_page(&self.client, "GET", &path, &[], page)
    }

    /// Adds a workspace member to the project. Returns `{ id, member, role }`, where `id`
    /// is the membership id that update and remove need.
    pub fn add_project_member(&self, project_id: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.proj(project_id, "project-members/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    /// `membership_id` is the id returned by `add_project_member` (not the user id).
    pub fn get_project_membership(
        &self,
        project_id: &Uuid,
        membership_id: &Uuid,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("project-members/{membership_id}/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn update_project_member(
        &self,
        project_id: &Uuid,
        membership_id: &Uuid,
        body: &Value,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("project-members/{membership_id}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    /// Deactivates the membership (CE 1.4.2 keeps the row with `is_active: false`).
    pub fn remove_project_member(
        &self,
        project_id: &Uuid,
        membership_id: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(project_id, &format!("project-members/{membership_id}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    pub fn list_invitations(&self) -> Result<Value, CoreError> {
        let path = self.ws("invitations/");
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    pub fn create_invitation(&self, body: &Value) -> Result<Value, CoreError> {
        let path = self.ws("invitations/");
        self.client.expect_json_post("POST", &path, body, 200..=299)
    }

    /// `PATCH invitations/{id}/`; the body may carry `role` and `email`.
    pub fn update_invitation(&self, pk: &Uuid, body: &Value) -> Result<Value, CoreError> {
        let path = self.ws(&format!("invitations/{pk}/"));
        self.client
            .expect_json_patch("PATCH", &path, body, 200..=299)
    }

    pub fn delete_invitation(&self, pk: &Uuid) -> Result<(), CoreError> {
        let path = self.ws(&format!("invitations/{pk}/"));
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }

    // --- work item attachments ---

    pub fn list_attachments(&self, project_id: &Uuid, issue_id: &Uuid) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/attachments/"));
        self.client.expect_json("GET", &path, &[], 200..=299)
    }

    /// Presigned download URL for an uploaded attachment (the API answers with a redirect).
    pub fn attachment_download_url(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<String, CoreError> {
        let path = self.proj(
            project_id,
            &format!("work-items/{issue_id}/attachments/{pk}/"),
        );
        self.client.get_redirect_location(&path)
    }

    pub fn download_presigned(&self, url: &str) -> Result<Vec<u8>, CoreError> {
        self.client.download_presigned(url)
    }

    /// Three-step upload: register the file (`POST attachments/` returns a presigned POST),
    /// send the bytes to storage, then mark the attachment uploaded (`PATCH is_uploaded`).
    /// Returns the `POST` response with `attachment.is_uploaded` set to true.
    pub fn upload_attachment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        file_name: &str,
        mime: &str,
        bytes: Vec<u8>,
    ) -> Result<Value, CoreError> {
        let path = self.proj(project_id, &format!("work-items/{issue_id}/attachments/"));
        let body = json!({ "name": file_name, "type": mime, "size": bytes.len() });
        let mut created = self
            .client
            .expect_json_post("POST", &path, &body, 200..=299)?;
        let asset_id = created
            .get("asset_id")
            .and_then(|x| x.as_str())
            .or_else(|| {
                created
                    .get("attachment")
                    .and_then(|a| a.get("id"))
                    .and_then(|x| x.as_str())
            })
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| {
                CoreError::UnexpectedResponse("attachment create response has no asset_id".into())
            })?;
        // Steps 2 and 3. On failure, delete the registered asset so no half-uploaded
        // attachment is left on the work item.
        let finish = || -> Result<(), (&'static str, CoreError)> {
            let upload = created.get("upload_data").cloned().unwrap_or(Value::Null);
            let url = upload.get("url").and_then(|x| x.as_str()).ok_or_else(|| {
                (
                    "register",
                    CoreError::UnexpectedResponse(
                        "attachment create response has no upload_data.url".into(),
                    ),
                )
            })?;
            let fields = upload
                .get("fields")
                .and_then(|x| x.as_object())
                .cloned()
                .unwrap_or_default();
            self.client
                .upload_presigned_post(url, &fields, file_name, mime, bytes)
                .map_err(|e| ("storage upload", e))?;
            let patch_path = self.proj(
                project_id,
                &format!("work-items/{issue_id}/attachments/{asset_id}/"),
            );
            self.client
                .expect_json_patch(
                    "PATCH",
                    &patch_path,
                    &json!({ "is_uploaded": true }),
                    200..=299,
                )
                .map_err(|e| ("mark uploaded", e))?;
            Ok(())
        };
        if let Err((step, err)) = finish() {
            let asset = asset_id.to_string();
            return Err(match self.delete_attachment(project_id, issue_id, &asset_id) {
                Ok(()) => err.with_details(
                    &format!("{step} failed; the registered attachment {asset} was deleted"),
                    json!({ "step": step, "asset_id": asset, "cleaned_up": true }),
                ),
                Err(cleanup) => err.with_details(
                    &format!(
                        "{step} failed and deleting the registered attachment {asset} also failed: {cleanup}; delete it with `work-items attachments delete --id {asset}`"
                    ),
                    json!({ "step": step, "asset_id": asset, "cleaned_up": false, "cleanup_error": cleanup.to_string() }),
                ),
            });
        }
        if let Some(obj) = created.as_object_mut() {
            // The presigned form fields are single-use secrets; do not echo them.
            obj.remove("upload_data");
            if let Some(att) = obj.get_mut("attachment").and_then(|a| a.as_object_mut()) {
                att.insert("is_uploaded".into(), json!(true));
            }
        }
        Ok(created)
    }

    pub fn delete_attachment(
        &self,
        project_id: &Uuid,
        issue_id: &Uuid,
        pk: &Uuid,
    ) -> Result<(), CoreError> {
        let path = self.proj(
            project_id,
            &format!("work-items/{issue_id}/attachments/{pk}/"),
        );
        self.client.expect_delete(&path, 200..=299)?;
        Ok(())
    }
}
