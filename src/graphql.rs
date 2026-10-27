//! A read-only GraphQL API of a model in the team hub (`POST /api/sources/{id}/graphql`): packages, elements with
//! attributes, operations, tagged values and relationships, and diagrams, for portals, reports and scripts.
//! Every list is limited; the hub's sign-in applies (any role).

use crate::config::AppConfig;
use juniper::{graphql_object, EmptyMutation, EmptySubscription, FieldResult, GraphQLObject, RootNode};

pub struct Ctx {
    pub cfg: AppConfig,
}
impl juniper::Context for Ctx {}

const MAX: i32 = 500;

fn rows(ctx: &Ctx, sql: &str) -> FieldResult<Vec<Vec<String>>> {
    let sql = crate::model_db::portable_sql(ctx.cfg.model_source, sql);
    crate::tools::query_rows(&ctx.cfg, &sql).map_err(|e| juniper::FieldError::new(format!("{e:#}"), juniper::Value::null()))
}

fn text(value: &str) -> String {
    value.replace('\'', "''")
}

fn limit(n: Option<i32>) -> i32 {
    n.unwrap_or(100).clamp(1, MAX)
}

#[derive(Clone)]
pub struct Package {
    id: i32,
    name: String,
    notes: String,
    parent_id: i32,
}

#[derive(Clone)]
pub struct Element {
    id: i32,
    name: String,
    kind: String,
    stereotype: String,
    status: String,
    notes: String,
    package_id: i32,
}

#[derive(Clone)]
pub struct Diagram {
    id: i32,
    name: String,
    kind: String,
    package_id: i32,
}

#[derive(Clone)]
pub struct Connector {
    id: i32,
    name: String,
    kind: String,
    stereotype: String,
    source_id: i32,
    target_id: i32,
}

#[derive(GraphQLObject, Clone)]
pub struct Attribute {
    name: String,
    r#type: String,
    notes: String,
}

#[derive(GraphQLObject, Clone)]
pub struct Operation {
    name: String,
    return_type: String,
    notes: String,
}

#[derive(GraphQLObject, Clone)]
pub struct TaggedValue {
    name: String,
    value: String,
}

fn int(v: &str) -> i32 {
    v.parse().unwrap_or(0)
}

const ELEMENT_COLUMNS: &str = "object_id, coalesce(name,''), object_type, coalesce(stereotype,''), coalesce(status,''), coalesce(note,''), package_id";

fn element_of(r: &[String]) -> Element {
    Element { id: int(&r[0]), name: r[1].clone(), kind: r[2].clone(), stereotype: r[3].clone(), status: r[4].clone(), notes: r[5].clone(), package_id: int(&r[6]) }
}

fn elements_where(ctx: &Ctx, condition: &str, n: i32) -> FieldResult<Vec<Element>> {
    Ok(rows(ctx, &format!("select {ELEMENT_COLUMNS} from t_object where {condition} order by name limit {n}"))?.iter().map(|r| element_of(r)).collect())
}

fn package_by(ctx: &Ctx, condition: &str, n: i32) -> FieldResult<Vec<Package>> {
    Ok(rows(ctx, &format!("select package_id, coalesce(name,''), coalesce(notes,''), parent_id from t_package where {condition} order by tpos, name limit {n}"))?
        .iter()
        .map(|r| Package { id: int(&r[0]), name: r[1].clone(), notes: r[2].clone(), parent_id: int(&r[3]) })
        .collect())
}

fn connectors_where(ctx: &Ctx, condition: &str) -> FieldResult<Vec<Connector>> {
    Ok(rows(ctx, &format!("select connector_id, coalesce(name,''), connector_type, coalesce(stereotype,''), start_object_id, end_object_id from t_connector where {condition} limit {MAX}"))?
        .iter()
        .map(|r| Connector { id: int(&r[0]), name: r[1].clone(), kind: r[2].clone(), stereotype: r[3].clone(), source_id: int(&r[4]), target_id: int(&r[5]) })
        .collect())
}

#[graphql_object(context = Ctx)]
impl Package {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn notes(&self) -> &str {
        &self.notes
    }
    fn parent_id(&self) -> i32 {
        self.parent_id
    }
    /// Subpackages.
    fn packages(&self, ctx: &Ctx) -> FieldResult<Vec<Package>> {
        package_by(ctx, &format!("parent_id = {}", self.id), MAX)
    }
    /// Elements directly in this package, optionally of one type.
    fn elements(&self, ctx: &Ctx, r#type: Option<String>, limit: Option<i32>) -> FieldResult<Vec<Element>> {
        let filter = r#type.map(|t| format!(" and object_type = '{}'", text(&t))).unwrap_or_default();
        elements_where(ctx, &format!("package_id = {} and object_type not in ('Package'){filter}", self.id), self::limit(limit))
    }
    fn diagrams(&self, ctx: &Ctx) -> FieldResult<Vec<Diagram>> {
        diagrams_where(ctx, &format!("package_id = {}", self.id))
    }
}

fn diagrams_where(ctx: &Ctx, condition: &str) -> FieldResult<Vec<Diagram>> {
    Ok(rows(ctx, &format!("select diagram_id, coalesce(name,''), diagram_type, package_id from t_diagram where {condition} order by name limit {MAX}"))?
        .iter()
        .map(|r| Diagram { id: int(&r[0]), name: r[1].clone(), kind: r[2].clone(), package_id: int(&r[3]) })
        .collect())
}

#[graphql_object(context = Ctx)]
impl Element {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    /// The UML / EA type (Class, Component, Requirement, Activity…).
    fn r#type(&self) -> &str {
        &self.kind
    }
    fn stereotype(&self) -> &str {
        &self.stereotype
    }
    fn status(&self) -> &str {
        &self.status
    }
    fn notes(&self) -> &str {
        &self.notes
    }
    fn package_id(&self) -> i32 {
        self.package_id
    }
    fn package(&self, ctx: &Ctx) -> FieldResult<Option<Package>> {
        Ok(package_by(ctx, &format!("package_id = {}", self.package_id), 1)?.into_iter().next())
    }
    fn attributes(&self, ctx: &Ctx) -> FieldResult<Vec<Attribute>> {
        Ok(rows(ctx, &format!("select coalesce(name,''), coalesce(type,''), coalesce(notes,'') from t_attribute where object_id = {} order by pos limit {MAX}", self.id))?
            .into_iter()
            .map(|r| Attribute { name: r[0].clone(), r#type: r[1].clone(), notes: r[2].clone() })
            .collect())
    }
    fn operations(&self, ctx: &Ctx) -> FieldResult<Vec<Operation>> {
        Ok(rows(ctx, &format!("select coalesce(name,''), coalesce(type,''), coalesce(notes,'') from t_operation where object_id = {} order by pos limit {MAX}", self.id))?
            .into_iter()
            .map(|r| Operation { name: r[0].clone(), return_type: r[1].clone(), notes: r[2].clone() })
            .collect())
    }
    fn tagged_values(&self, ctx: &Ctx) -> FieldResult<Vec<TaggedValue>> {
        Ok(rows(ctx, &format!("select coalesce(property,''), coalesce(value,'') from t_objectproperties where object_id = {} limit {MAX}", self.id))?
            .into_iter()
            .map(|r| TaggedValue { name: r[0].clone(), value: r[1].clone() })
            .collect())
    }
    /// Relationships starting or ending at this element.
    fn connectors(&self, ctx: &Ctx) -> FieldResult<Vec<Connector>> {
        connectors_where(ctx, &format!("start_object_id = {0} or end_object_id = {0}", self.id))
    }
}

#[graphql_object(context = Ctx)]
impl Diagram {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn r#type(&self) -> &str {
        &self.kind
    }
    fn package_id(&self) -> i32 {
        self.package_id
    }
    /// The elements shown on the diagram.
    fn elements(&self, ctx: &Ctx) -> FieldResult<Vec<Element>> {
        elements_where(ctx, &format!("object_id in (select object_id from t_diagramobjects where diagram_id = {})", self.id), MAX)
    }
}

#[graphql_object(context = Ctx)]
impl Connector {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn r#type(&self) -> &str {
        &self.kind
    }
    fn stereotype(&self) -> &str {
        &self.stereotype
    }
    fn source(&self, ctx: &Ctx) -> FieldResult<Option<Element>> {
        Ok(elements_where(ctx, &format!("object_id = {}", self.source_id), 1)?.into_iter().next())
    }
    fn target(&self, ctx: &Ctx) -> FieldResult<Option<Element>> {
        Ok(elements_where(ctx, &format!("object_id = {}", self.target_id), 1)?.into_iter().next())
    }
}

pub struct Query;

#[graphql_object(context = Ctx)]
impl Query {
    /// Packages under a parent (0 = the model roots).
    fn packages(ctx: &Ctx, parent: Option<i32>) -> FieldResult<Vec<Package>> {
        package_by(ctx, &format!("parent_id = {}", parent.unwrap_or(0)), MAX)
    }
    fn package(ctx: &Ctx, id: i32) -> FieldResult<Option<Package>> {
        Ok(package_by(ctx, &format!("package_id = {id}"), 1)?.into_iter().next())
    }
    /// Elements by name (contains, any letter case), type, stereotype and package.
    fn elements(ctx: &Ctx, name: Option<String>, r#type: Option<String>, stereotype: Option<String>, package: Option<i32>, limit: Option<i32>) -> FieldResult<Vec<Element>> {
        let mut condition = vec!["object_type <> 'Package'".to_string()];
        if let Some(n) = name {
            condition.push(format!("lower(name) like '%{}%'", text(&n.to_lowercase())));
        }
        if let Some(t) = r#type {
            condition.push(format!("object_type = '{}'", text(&t)));
        }
        if let Some(s) = stereotype {
            condition.push(format!("stereotype = '{}'", text(&s)));
        }
        if let Some(p) = package {
            condition.push(format!("package_id = {p}"));
        }
        elements_where(ctx, &condition.join(" and "), self::limit(limit))
    }
    fn element(ctx: &Ctx, id: i32) -> FieldResult<Option<Element>> {
        Ok(elements_where(ctx, &format!("object_id = {id}"), 1)?.into_iter().next())
    }
    fn diagrams(ctx: &Ctx, package: Option<i32>, name: Option<String>) -> FieldResult<Vec<Diagram>> {
        let mut condition = vec!["1 = 1".to_string()];
        if let Some(p) = package {
            condition.push(format!("package_id = {p}"));
        }
        if let Some(n) = name {
            condition.push(format!("lower(name) like '%{}%'", text(&n.to_lowercase())));
        }
        diagrams_where(ctx, &condition.join(" and "))
    }
    fn diagram(ctx: &Ctx, id: i32) -> FieldResult<Option<Diagram>> {
        Ok(diagrams_where(ctx, &format!("diagram_id = {id}"))?.into_iter().next())
    }
}

pub type Schema = RootNode<'static, Query, EmptyMutation<Ctx>, EmptySubscription<Ctx>>;

/// Runs one GraphQL request ({"query", "variables", "operationName"}) and gives the JSON answer ({"data", "errors"}).
pub fn execute(cfg: &AppConfig, request: &serde_json::Value) -> serde_json::Value {
    let schema = Schema::new(Query, EmptyMutation::new(), EmptySubscription::new());
    let query = request["query"].as_str().unwrap_or("");
    if query.len() > 20_000 {
        return serde_json::json!({ "errors": [{ "message": "the query is too long" }] });
    }
    let variables: juniper::Variables = serde_json::from_value(request["variables"].clone()).unwrap_or_default();
    let ctx = Ctx { cfg: cfg.clone() };
    match juniper::execute_sync(query, request["operationName"].as_str(), &schema, &variables, &ctx) {
        Ok((data, errors)) => serde_json::json!({
            "data": serde_json::to_value(&data).unwrap_or_default(),
            "errors": errors.iter().map(|e| serde_json::json!({ "message": e.error().message(), "path": e.path() })).collect::<Vec<_>>(),
        }),
        Err(e) => serde_json::json!({ "errors": [{ "message": e.to_string() }] }),
    }
}

/// The schema in SDL (shown to people who write queries).
pub fn sdl() -> String {
    Schema::new(Query, EmptyMutation::new(), EmptySubscription::new()).as_sdl()
}
