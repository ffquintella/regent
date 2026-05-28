use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashSet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum PuppetValue {
    String(String),
    Integer(i64),
    Bool(bool),
    Array(Vec<PuppetValue>),
    Hash(HashMap<String, PuppetValue>),
    Undef,
}

impl PuppetValue {
    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => PuppetValue::Undef,
            serde_json::Value::Bool(value) => PuppetValue::Bool(*value),
            serde_json::Value::Number(value) => {
                PuppetValue::Integer(value.as_i64().unwrap_or_default())
            }
            serde_json::Value::String(value) => PuppetValue::String(value.clone()),
            serde_json::Value::Array(values) => {
                PuppetValue::Array(values.iter().map(PuppetValue::from_json).collect())
            }
            serde_json::Value::Object(map) => PuppetValue::Hash(
                map.iter()
                    .map(|(key, value)| (key.clone(), PuppetValue::from_json(value)))
                    .collect(),
            ),
        }
    }

    pub fn from_json_map(map: Option<&HashMap<String, serde_json::Value>>) -> PuppetValue {
        let Some(map) = map else {
            return PuppetValue::Hash(HashMap::new());
        };
        PuppetValue::Hash(
            map.iter()
                .map(|(key, value)| (key.clone(), PuppetValue::from_json(value)))
                .collect(),
        )
    }

    pub fn as_string(&self) -> String {
        match self {
            PuppetValue::String(value) => value.clone(),
            PuppetValue::Integer(value) => value.to_string(),
            PuppetValue::Bool(value) => value.to_string(),
            PuppetValue::Array(values) => {
                let items = values.iter().map(|value| value.as_string()).collect::<Vec<_>>();
                format!("[{}]", items.join(", "))
            }
            PuppetValue::Hash(map) => {
                let items = map
                    .iter()
                    .map(|(key, value)| format!("{key}=>{}", value.as_string()))
                    .collect::<Vec<_>>();
                format!("{{{}}}", items.join(", "))
            }
            PuppetValue::Undef => "undef".to_string(),
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            PuppetValue::Bool(value) => *value,
            PuppetValue::Undef => false,
            PuppetValue::String(value) => !value.is_empty(),
            PuppetValue::Array(value) => !value.is_empty(),
            PuppetValue::Hash(value) => !value.is_empty(),
            PuppetValue::Integer(_) => true,
        }
    }

    fn downcase(&self) -> PuppetValue {
        PuppetValue::String(self.as_string().to_lowercase())
    }
}

#[derive(Debug, Clone)]
pub struct PuppetResource {
    pub resource_type: String,
    pub title: String,
    pub attributes: HashMap<String, PuppetValue>,
}

#[derive(Debug, Clone, Default)]
pub struct PuppetCatalog {
    resources: HashMap<String, HashMap<String, PuppetResource>>,
}

impl PuppetCatalog {
    pub fn add(&mut self, resource: PuppetResource) {
        self.resources
            .entry(resource.resource_type.clone())
            .or_default()
            .insert(resource.title.clone(), resource);
    }

    pub fn contains(&self, resource_type: &str, title: &str) -> bool {
        self.resources
            .get(resource_type)
            .and_then(|entries| entries.get(title))
            .is_some()
    }

    pub fn find(&self, resource_type: &str, title: &str) -> Option<&PuppetResource> {
        self.resources.get(resource_type).and_then(|map| map.get(title))
    }
}

pub struct PuppetEvaluator {
    module: PuppetModule,
}

/// Names of Puppet classes and defines actually entered during a single
/// catalog evaluation. Used to attribute coverage back to source files.
#[derive(Debug, Default, Clone)]
pub struct EvaluationTrace {
    pub classes: HashSet<String>,
    pub defines: HashSet<String>,
}

impl PuppetEvaluator {
    pub fn new(module_path: &Path) -> Result<Self> {
        let fixtures = discover_fixture_module_paths(module_path);
        let module = PuppetModule::load_with_fixtures(module_path, &fixtures)?;
        Ok(Self { module })
    }

    pub fn is_define(&self, name: &str) -> bool {
        self.module.defines.contains_key(name)
    }

    pub fn class_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.module.classes.keys().cloned().collect();
        names.sort();
        names
    }

    /// `.pp` files discovered under the primary module's `manifests/` tree.
    /// Fixture modules are excluded — coverage only reports on code owned by
    /// the module under test.
    pub fn primary_manifest_files(&self) -> &[PathBuf] {
        &self.module.primary_manifest_files
    }

    /// Returns the source file that defined the named class, if any.
    /// `None` for classes that came in via a fixture module (no origin recorded
    /// for fixtures) or for unknown names.
    pub fn class_origin_file(&self, name: &str) -> Option<&Path> {
        self.module
            .classes
            .get(name)
            .and_then(|def| def.origin_file.as_deref())
    }

    pub fn define_origin_file(&self, name: &str) -> Option<&Path> {
        self.module
            .defines
            .get(name)
            .and_then(|def| def.origin_file.as_deref())
    }

    pub fn evaluate_class(
        &self,
        name: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<PuppetCatalog> {
        self.evaluate_class_traced(name, facts, params).map(|(c, _)| c)
    }

    pub fn evaluate_define(
        &self,
        name: &str,
        title: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<PuppetCatalog> {
        self.evaluate_define_traced(name, title, facts, params)
            .map(|(c, _)| c)
    }

    pub fn evaluate_class_traced(
        &self,
        name: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<(PuppetCatalog, EvaluationTrace)> {
        let mut ctx = EvalContext::new(facts.clone(), params.clone(), &self.module);
        ctx.evaluate_class(name)?;
        let catalog = std::mem::take(&mut ctx.catalog);
        Ok((catalog, ctx.into_trace()))
    }

    pub fn evaluate_define_traced(
        &self,
        name: &str,
        title: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<(PuppetCatalog, EvaluationTrace)> {
        let mut ctx = EvalContext::new(facts.clone(), params.clone(), &self.module);
        ctx.evaluate_define(name, title)?;
        let catalog = std::mem::take(&mut ctx.catalog);
        Ok((catalog, ctx.into_trace()))
    }
}

#[derive(Debug, Clone)]
struct PuppetModule {
    classes: HashMap<String, ClassDef>,
    defines: HashMap<String, DefineDef>,
    /// Maps a Puppet module name (the basename of a loaded module dir) to its
    /// filesystem path, so `epp('foo/bar.epp')` can resolve to
    /// `<paths["foo"]>/templates/bar.epp`.
    module_paths: HashMap<String, std::path::PathBuf>,
    /// All `.pp` manifest files discovered under the primary module's
    /// manifests/ tree (fixture modules excluded). Used to compute the coverage
    /// denominator.
    primary_manifest_files: Vec<PathBuf>,
}

impl PuppetModule {
    fn load_with_fixtures(module_path: &Path, fixture_module_paths: &[std::path::PathBuf]) -> Result<Self> {
        let mut classes = HashMap::new();
        let mut defines = HashMap::new();
        let mut module_paths: HashMap<String, std::path::PathBuf> = HashMap::new();

        // Load fixtures first so the primary module's defs win on conflict.
        for fixture_path in fixture_module_paths {
            register_module_path(&mut module_paths, fixture_path);
            // Be tolerant: a single broken fixture should not block the run.
            let mut sink = Vec::new();
            if let Err(err) = load_manifests_into(fixture_path, &mut classes, &mut defines, &mut sink) {
                eprintln!(
                    "warning: skipping fixture module {}: {}",
                    fixture_path.display(),
                    err
                );
            }
        }

        register_module_path(&mut module_paths, &module_path.to_path_buf());
        let mut primary_manifest_files = Vec::new();
        load_manifests_into(module_path, &mut classes, &mut defines, &mut primary_manifest_files)
            .with_context(|| format!("load manifests for {}", module_path.display()))?;
        primary_manifest_files.sort();

        Ok(Self { classes, defines, module_paths, primary_manifest_files })
    }
}

/// Index `module_path` under every plausible Puppet module name so that
/// template references (`epp('foo/bar.epp')`) resolve regardless of how the
/// module dir is named on disk. Tries:
///   * `metadata.json` "name" field (after the last `-` or `/`)
///   * the dir basename, with a leading `puppet-` stripped
///   * the raw dir basename
fn register_module_path(
    paths: &mut HashMap<String, std::path::PathBuf>,
    module_path: &std::path::PathBuf,
) {
    let mut insert = |name: &str| {
        if !name.is_empty() {
            paths
                .entry(name.to_string())
                .or_insert_with(|| module_path.clone());
        }
    };
    if let Ok(contents) = std::fs::read_to_string(module_path.join("metadata.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
                let bare = name
                    .rsplit_once('-')
                    .map(|(_, tail)| tail)
                    .or_else(|| name.rsplit_once('/').map(|(_, tail)| tail))
                    .unwrap_or(name);
                insert(bare);
            }
        }
    }
    if let Some(basename) = module_path.file_name().and_then(|n| n.to_str()) {
        insert(basename);
        if let Some(stripped) = basename.strip_prefix("puppet-") {
            insert(stripped);
        }
    }
}

fn load_manifests_into(
    module_path: &Path,
    classes: &mut HashMap<String, ClassDef>,
    defines: &mut HashMap<String, DefineDef>,
    discovered_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let manifest_dir = module_path.join("manifests");
    if !manifest_dir.exists() {
        return Ok(());
    }
    let mut stack = vec![manifest_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("read manifest dir {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("pp") {
                continue;
            }
            discovered_files.push(path.clone());
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("read manifest {}", path.display()))?;
            let mut parser = PuppetParser::new(&content);
            let defs = parser.parse_definitions().with_context(|| {
                format!("parse manifest {}:{}", path.display(), parser.position())
            })?;
            for warning in &parser.warnings {
                let (line, col) = source_line_col(&content, warning.offset);
                eprintln!(
                    "warning: {}:{}:{}: {}",
                    path.display(),
                    line,
                    col,
                    warning.message
                );
            }
            for def in defs {
                match def {
                    PuppetDef::Class(mut def) => {
                        def.origin_file = Some(path.clone());
                        classes.insert(def.name.clone(), def);
                    }
                    PuppetDef::Define(mut def) => {
                        def.origin_file = Some(path.clone());
                        defines.insert(def.name.clone(), def);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Discover modules under `spec/fixtures/modules/` for inclusion in the evaluator's namespace.
/// Each immediate subdirectory is treated as a separate Puppet module root.
fn discover_fixture_module_paths(module_path: &Path) -> Vec<std::path::PathBuf> {
    let modules_dir = module_path.join("spec").join("fixtures").join("modules");
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir(&modules_dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Follow symlinks: they're standard for self-referential fixtures.
        let is_dir = std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        // Skip the self-symlink to the module being tested.
        if path.canonicalize().ok() == module_path.canonicalize().ok() {
            continue;
        }
        result.push(path);
    }
    result
}

#[derive(Debug, Clone)]
struct ClassDef {
    name: String,
    params: HashMap<String, Expr>,
    parent: Option<String>,
    body: Vec<Stmt>,
    origin_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct DefineDef {
    name: String,
    params: HashMap<String, Expr>,
    body: Vec<Stmt>,
    origin_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum PuppetDef {
    Class(ClassDef),
    Define(DefineDef),
}

#[derive(Debug, Clone)]
enum Stmt {
    VarAssign(String, Expr),
    Resource {
        rtype: String,
        titles: Vec<Expr>,
        attrs: HashMap<String, Expr>,
    },
    Include(String),
    If {
        cond: Cond,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    Case {
        expr: Expr,
        branches: Vec<(Expr, Vec<Stmt>)>,
        default: Vec<Stmt>,
    },
    Fail(String),
    /// Placeholder for statements the parser recognises and consumes but the
    /// evaluator does not need to act on (e.g. resource collectors
    /// `Foo<| ... |>`).
    Noop,
}

#[derive(Debug, Clone)]
enum Cond {
    Not(Box<Cond>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Defined { rtype: String, title: String },
    Compare { left: Expr, op: CompareOp, right: Expr },
    /// `<needle> in <haystack>` — membership test against arrays, hash keys, or
    /// substring of a string.
    In { needle: Expr, haystack: Expr },
    Truthy(Expr),
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    NotEq,
    Match,
    NotMatch,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

#[derive(Debug, Clone)]
enum Expr {
    String(String),
    Integer(i64),
    Bool(bool),
    Undef,
    Array(Vec<Expr>),
    Hash(Vec<(Expr, Expr)>),
    Var(VarRef),
    MethodCall { target: Box<Expr>, name: String },
    ResourceRef { rtype: String, title: Box<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    /// Puppet regex literal: `/pattern/[flags]`. Stored as the inner pattern;
    /// flags are pre-applied as inline modifiers (e.g. `(?i)`).
    Regex(String),
    /// Puppet selector: `subject ? { case_key => value, ..., default => value }`.
    /// A bare `Expr::String("default")` case key is the fallback.
    Selector {
        subject: Box<Expr>,
        cases: Vec<(Expr, Expr)>,
    },
    /// Boolean/comparison expression used in value position (e.g. RHS of
    /// `$x = $a and $b == 'foo'`). Evaluates to a `Bool`.
    Condition(Box<Cond>),
    /// Binary arithmetic (`+ - * /`). For integer operands the result is
    /// computed; otherwise we fall back to the left-hand value so the parse
    /// succeeds and the evaluator stays usable for downstream code paths.
    Arith {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy)]
enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
struct VarRef {
    name: String,
    path: Vec<String>,
}

struct EvalContext<'a> {
    vars: HashMap<String, PuppetValue>,
    facts: PuppetValue,
    params: PuppetValue,
    catalog: PuppetCatalog,
    module: &'a PuppetModule,
    in_progress: Vec<String>,
    class_stack: Vec<String>,
    evaluated_classes: HashSet<String>,
    evaluated_defines: HashSet<String>,
}

impl<'a> EvalContext<'a> {
    fn new(facts: PuppetValue, params: PuppetValue, module: &'a PuppetModule) -> Self {
        Self {
            vars: HashMap::new(),
            facts,
            params,
            catalog: PuppetCatalog::default(),
            module,
            in_progress: Vec::new(),
            class_stack: Vec::new(),
            evaluated_classes: HashSet::new(),
            evaluated_defines: HashSet::new(),
        }
    }

    fn into_trace(self) -> EvaluationTrace {
        EvaluationTrace {
            classes: self.evaluated_classes,
            defines: self.evaluated_defines,
        }
    }

    fn evaluate_class(&mut self, name: &str) -> Result<()> {
        if self.evaluated_classes.contains(name) {
            return Ok(());
        }
        if self.in_progress.contains(&name.to_string()) {
            return Ok(());
        }
        let class_def = self
            .module
            .classes
            .get(name)
            .with_context(|| format!("class {name} not found"))?
            .clone();
        self.in_progress.push(name.to_string());

        let mut local_vars = HashMap::new();
        if let Some(parent) = &class_def.parent {
            self.evaluate_class(parent)?;
            for (key, value) in self.vars.clone() {
                if key.starts_with(&format!("{parent}::")) {
                    local_vars.insert(key, value);
                }
            }
        }
        self.apply_param_defaults(&class_def.params, &mut local_vars)?;
        self.apply_param_overrides(&mut local_vars)?;
        self.vars.extend(local_vars);

        self.catalog.add(PuppetResource {
            resource_type: "class".to_string(),
            title: name.to_string(),
            attributes: HashMap::new(),
        });

        self.class_stack.push(name.to_string());
        self.evaluate_statements(&class_def.body)?;
        self.class_stack.pop();
        self.evaluated_classes.insert(name.to_string());
        self.in_progress.retain(|item| item != name);
        Ok(())
    }

    /// Expand a defined-type resource declared inside a class/define body.
    /// Saves/restores the surrounding scope's vars so this works recursively.
    fn instantiate_define(
        &mut self,
        name: &str,
        title: &str,
        attrs: &HashMap<String, PuppetValue>,
    ) -> Result<()> {
        let key = format!("{name}[{title}]");
        if self.in_progress.contains(&key) {
            return Ok(());
        }
        let define_def = self
            .module
            .defines
            .get(name)
            .with_context(|| format!("define {name} not found"))?
            .clone();
        self.in_progress.push(key.clone());
        let saved_vars = self.vars.clone();
        self.vars.insert("title".to_string(), PuppetValue::String(title.to_string()));
        self.vars.insert("name".to_string(), PuppetValue::String(title.to_string()));
        let mut local_vars = HashMap::new();
        self.apply_param_defaults(&define_def.params, &mut local_vars)?;
        for (key, value) in attrs {
            local_vars.insert(key.clone(), value.clone());
        }
        self.vars.extend(local_vars);
        let result = self.evaluate_statements(&define_def.body);
        self.vars = saved_vars;
        self.in_progress.retain(|item| item != &key);
        self.evaluated_defines.insert(name.to_string());
        result
    }

    fn evaluate_define(&mut self, name: &str, title: &str) -> Result<()> {
        let define_def = self
            .module
            .defines
            .get(name)
            .with_context(|| format!("define {name} not found"))?
            .clone();
        self.evaluated_defines.insert(name.to_string());
        self.vars
            .insert("title".to_string(), PuppetValue::String(title.to_string()));
        let mut local_vars = HashMap::new();
        self.apply_param_defaults(&define_def.params, &mut local_vars)?;
        self.apply_param_overrides(&mut local_vars)?;
        self.vars.extend(local_vars);
        self.evaluate_statements(&define_def.body)?;
        Ok(())
    }

    fn apply_param_defaults(
        &mut self,
        defaults: &HashMap<String, Expr>,
        vars: &mut HashMap<String, PuppetValue>,
    ) -> Result<()> {
        for (name, expr) in defaults {
            let value = self.eval_expr(expr)?;
            vars.insert(name.clone(), value);
        }
        Ok(())
    }

    fn apply_param_overrides(&mut self, vars: &mut HashMap<String, PuppetValue>) -> Result<()> {
        let PuppetValue::Hash(params) = &self.params else {
            return Ok(());
        };
        for (key, value) in params {
            vars.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    fn evaluate_statements(&mut self, statements: &[Stmt]) -> Result<()> {
        for stmt in statements {
            match stmt {
                Stmt::VarAssign(name, expr) => {
                    let value = self.eval_expr(expr)?;
                    self.vars.insert(name.clone(), value);
                    if let Some(scope) = self.class_stack.last() {
                        let scoped = format!("{scope}::{name}");
                        let scoped_value = self.vars.get(name).cloned().unwrap_or(PuppetValue::Undef);
                        self.vars.insert(scoped, scoped_value);
                    }
                }
                Stmt::Resource { rtype, titles, attrs } => {
                    let mut title_values = Vec::new();
                    for title_expr in titles {
                        let value = self.eval_expr(title_expr)?;
                        match value {
                            PuppetValue::Array(values) => {
                                for item in values {
                                    title_values.push(item.as_string());
                                }
                            }
                            _ => title_values.push(value.as_string()),
                        }
                    }
                    let mut attributes = HashMap::new();
                    for (key, expr) in attrs {
                        attributes.insert(key.clone(), self.eval_expr(expr)?);
                    }
                    for title in title_values {
                        let resource_type = normalize_rtype(rtype);
                        let resource = PuppetResource {
                            resource_type: resource_type.clone(),
                            title: title.clone(),
                            attributes: attributes.clone(),
                        };
                        self.catalog.add(resource);
                        if resource_type == "class" {
                            let _ = self.evaluate_class(&title);
                        } else if self.module.defines.contains_key(&resource_type) {
                            let _ = self.instantiate_define(&resource_type, &title, &attributes);
                        }
                    }
                }
                Stmt::Include(name) => {
                    let _ = self.evaluate_class(name);
                }
                Stmt::If { cond, then_body, else_body } => {
                    if self.eval_cond(cond)? {
                        self.evaluate_statements(then_body)?;
                    } else {
                        self.evaluate_statements(else_body)?;
                    }
                }
                Stmt::Case { expr, branches, default } => {
                    let value = self.eval_expr(expr)?;
                    let mut matched = false;
                    for (branch_expr, body) in branches {
                        if self.case_branch_matches(branch_expr, &value)? {
                            self.evaluate_statements(body)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        self.evaluate_statements(default)?;
                    }
                }
                Stmt::Fail(message) => {
                    return Err(anyhow::anyhow!(message.clone()));
                }
                Stmt::Noop => {}
            }
        }
        Ok(())
    }

    /// Match a `case` branch pattern against the subject value.
    ///
    /// Supported branch patterns:
    /// - bare regex literal `/.../[imsx]` — substring match on the subject
    /// - array of patterns `[a, b, /c/, ...]` — true if any element matches
    /// - any other expression — equality after evaluation
    fn case_branch_matches(&mut self, pattern: &Expr, subject: &PuppetValue) -> Result<bool> {
        match pattern {
            Expr::Regex(regex_src) => eval_regex_match(
                subject,
                &PuppetValue::String(regex_src.clone()),
            ),
            Expr::Array(items) => {
                for item in items {
                    if self.case_branch_matches(item, subject)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            other => Ok(self.eval_expr(other)? == *subject),
        }
    }

    fn eval_cond(&mut self, cond: &Cond) -> Result<bool> {
        Ok(match cond {
            Cond::Not(inner) => !self.eval_cond(inner)?,
            Cond::And(left, right) => self.eval_cond(left)? && self.eval_cond(right)?,
            Cond::Or(left, right) => self.eval_cond(left)? || self.eval_cond(right)?,
            Cond::Defined { rtype, title } => {
                let rtype = normalize_rtype(rtype);
                self.catalog.contains(&rtype, title)
            }
            Cond::Compare { left, op, right } => {
                match op {
                    CompareOp::Eq => self.eval_expr(left)? == self.eval_expr(right)?,
                    CompareOp::NotEq => self.eval_expr(left)? != self.eval_expr(right)?,
                    CompareOp::Match => eval_regex_match(
                        &self.eval_expr(left)?,
                        &self.eval_expr(right)?,
                    )?,
                    CompareOp::NotMatch => !eval_regex_match(
                        &self.eval_expr(left)?,
                        &self.eval_expr(right)?,
                    )?,
                    CompareOp::Lt | CompareOp::Gt | CompareOp::LtEq | CompareOp::GtEq => {
                        eval_ordered_compare(&self.eval_expr(left)?, *op, &self.eval_expr(right)?)
                    }
                }
            }
            Cond::In { needle, haystack } => {
                let needle = self.eval_expr(needle)?;
                let haystack = self.eval_expr(haystack)?;
                eval_in(&needle, &haystack)
            }
            Cond::Truthy(expr) => self.eval_expr(expr)?.is_truthy(),
        })
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<PuppetValue> {
        Ok(match expr {
            Expr::String(value) => PuppetValue::String(self.expand_string(value)?),
            Expr::Integer(value) => PuppetValue::Integer(*value),
            Expr::Bool(value) => PuppetValue::Bool(*value),
            Expr::Undef => PuppetValue::Undef,
            Expr::Array(values) => PuppetValue::Array(
                values
                    .iter()
                    .map(|value| self.eval_expr(value))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Expr::Hash(values) => {
                let mut map = HashMap::new();
                for (key, value) in values {
                    let key = self.eval_expr(key)?.as_string();
                    map.insert(key, self.eval_expr(value)?);
                }
                PuppetValue::Hash(map)
            }
            Expr::Var(var) => self.resolve_var(var),
            Expr::Condition(cond) => PuppetValue::Bool(self.eval_cond(cond)?),
            Expr::Arith { op, left, right } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                eval_arith(&l, *op, &r)
            }
            Expr::MethodCall { target, name } => {
                let value = self.eval_expr(target)?;
                match name.as_str() {
                    "downcase" => value.downcase(),
                    _ => value,
                }
            }
            Expr::ResourceRef { rtype, title } => {
                let title = self.eval_expr(title)?.as_string();
                PuppetValue::String(format!("{rtype}[{title}]"))
            }
            Expr::FunctionCall { name, args } => {
                let arg_values: Vec<PuppetValue> = args
                    .iter()
                    .map(|arg| self.eval_expr(arg))
                    .collect::<Result<Vec<_>>>()?;
                match name.as_str() {
                    "epp" => {
                        let template_ref = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        let params = arg_values
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| PuppetValue::Hash(HashMap::new()));
                        match self.resolve_template_file(&template_ref) {
                            Some(path) => match std::fs::read_to_string(&path) {
                                Ok(template) => PuppetValue::String(render_epp(&template, &params)),
                                Err(_) => PuppetValue::String(format!("<epp:{template_ref}>")),
                            },
                            None => PuppetValue::String(format!("<epp:{template_ref}>")),
                        }
                    }
                    "inline_epp" => {
                        let template = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        let params = arg_values
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| PuppetValue::Hash(HashMap::new()));
                        PuppetValue::String(render_epp(&template, &params))
                    }
                    "template" | "inline_template" => {
                        // ERB templates aren't supported yet; fall back to a
                        // placeholder so failures surface as content mismatches
                        // rather than crashes.
                        let source = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        PuppetValue::String(format!("<{name}:{source}>"))
                    }
                    _ => PuppetValue::Undef,
                }
            }
            // Regex literals evaluate to their pattern string; comparison ops
            // `=~` / `!~` compile and apply the pattern.
            Expr::Regex(pattern) => PuppetValue::String(pattern.clone()),
            Expr::Selector { subject, cases } => {
                let subject_value = self.eval_expr(subject)?;
                let mut default_value: Option<&Expr> = None;
                let mut matched: Option<&Expr> = None;
                for (key, value) in cases {
                    if let Expr::String(literal) = key {
                        if literal == "default" {
                            default_value = Some(value);
                            continue;
                        }
                    }
                    if let Expr::Regex(_) = key {
                        let pattern = self.eval_expr(key)?;
                        if eval_regex_match(&subject_value, &pattern).unwrap_or(false) {
                            matched = Some(value);
                            break;
                        }
                        continue;
                    }
                    let case_value = self.eval_expr(key)?;
                    if case_value == subject_value {
                        matched = Some(value);
                        break;
                    }
                }
                let chosen = matched.or(default_value);
                match chosen {
                    Some(expr) => self.eval_expr(expr)?,
                    None => PuppetValue::Undef,
                }
            }
        })
    }

    /// Resolve a Puppet template reference like `"rustion/rustion.toml.epp"`
    /// to an absolute filesystem path of the form `<module>/templates/<rest>`.
    fn resolve_template_file(&self, reference: &str) -> Option<std::path::PathBuf> {
        let (module_name, rest) = reference.split_once('/')?;
        let module_root = self.module.module_paths.get(module_name)?;
        Some(module_root.join("templates").join(rest))
    }

    fn resolve_var(&self, var: &VarRef) -> PuppetValue {
        // Strip the legacy top-scope `::` prefix (e.g. `$::osfamily`).
        let normalized = var.name.trim_start_matches(':').to_string();
        let mut value = if var.name == "facts" {
            self.facts.clone()
        } else if let Some(v) = self.vars.get(&var.name).cloned() {
            v
        } else if let Some(v) = self.vars.get(&normalized).cloned() {
            v
        } else if let PuppetValue::Hash(facts) = &self.facts {
            // Legacy top-scope fact references: `$::osfamily` → `$facts['osfamily']`.
            facts.get(&normalized).cloned().unwrap_or(PuppetValue::Undef)
        } else {
            PuppetValue::Undef
        };
        for segment in &var.path {
            value = match value {
                PuppetValue::Hash(map) => map.get(segment).cloned().unwrap_or(PuppetValue::Undef),
                _ => PuppetValue::Undef,
            };
        }
        value
    }

    fn expand_string(&mut self, input: &str) -> Result<String> {
        let mut output = String::new();
        let mut remaining = input;
        while let Some(start) = remaining.find("${") {
            let (prefix, rest) = remaining.split_at(start);
            output.push_str(prefix);
            let Some(end) = rest.find('}') else {
                break;
            };
            let expr_source = &rest[2..end];
            let expr = PuppetParser::parse_inline_expr(expr_source)?;
            let value = self.eval_expr(&expr)?;
            output.push_str(&value.as_string());
            remaining = &rest[end + 1..];
        }
        output.push_str(remaining);
        Ok(output)
    }
}

fn normalize_rtype(rtype: &str) -> String {
    rtype.to_lowercase().replace("__", "::")
}

struct PuppetParser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    index: usize,
    allow_bare_vars: bool,
    warnings: Vec<ParseWarning>,
}

#[derive(Debug, Clone)]
struct ParseWarning {
    offset: usize,
    message: String,
}

impl<'a> PuppetParser<'a> {
    fn new(source: &'a str) -> Self {
        let tokens = Lexer::new(source).tokenize();
        Self {
            source,
            tokens,
            index: 0,
            allow_bare_vars: false,
            warnings: Vec::new(),
        }
    }

    /// If the next two tokens are `:` `:`, consume them and record a deprecation
    /// warning. Returns `true` when a legacy leading `::` prefix was stripped.
    ///
    /// Puppet historically allowed a leading `::` to mean "top-level scope" on
    /// class names and type references (`include ::ntp`, `inherits ::base`,
    /// `::apache::vhost { ... }`). Modern Puppet treats the prefix as a no-op
    /// and lint rules flag it. Regent accepts it for compatibility but warns so
    /// the user can clean it up.
    fn consume_leading_namespace_prefix(&mut self) -> bool {
        let first = self.tokens.get(self.index);
        let second = self.tokens.get(self.index + 1);
        let (Some(a), Some(b)) = (first, second) else {
            return false;
        };
        if a.kind != TokenKind::Colon || b.kind != TokenKind::Colon {
            return false;
        }
        let offset = a.offset;
        self.index += 2;
        self.warnings.push(ParseWarning {
            offset,
            message:
                "deprecated leading `::` namespace prefix; modern Puppet treats it as a no-op"
                    .to_string(),
        });
        true
    }

    fn position(&self) -> usize {
        if let Some(token) = self.tokens.get(self.index) {
            token.offset
        } else {
            self.source.len()
        }
    }

    fn parse_definitions(&mut self) -> Result<Vec<PuppetDef>> {
        let mut defs = Vec::new();
        while !self.is_eof() {
            if self.consume_keyword("class") {
                defs.push(PuppetDef::Class(self.parse_class_def()?));
            } else if self.consume_keyword("define") {
                defs.push(PuppetDef::Define(self.parse_define_def()?));
            } else {
                self.index += 1;
            }
        }
        Ok(defs)
    }

    fn parse_class_def(&mut self) -> Result<ClassDef> {
        let name = self.expect_ident()?;
        let params = self.parse_param_list()?;
        let parent = if self.consume_keyword("inherits") {
            self.consume_leading_namespace_prefix();
            Some(self.expect_ident()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(ClassDef { name, params, parent, body, origin_file: None })
    }

    fn parse_define_def(&mut self) -> Result<DefineDef> {
        let name = self.expect_ident()?;
        let params = self.parse_param_list()?;
        let body = self.parse_block()?;
        Ok(DefineDef { name, params, body, origin_file: None })
    }

    fn parse_param_list(&mut self) -> Result<HashMap<String, Expr>> {
        let mut params = HashMap::new();
        if !self.consume(TokenKind::LParen) {
            return Ok(params);
        }
        while !self.consume(TokenKind::RParen) && !self.is_eof() {
            if self.peek_kind() == Some(TokenKind::Comma) {
                self.index += 1;
                continue;
            }
            while self.peek_kind() != Some(TokenKind::Var)
                && self.peek_kind() != Some(TokenKind::Comma)
                && self.peek_kind() != Some(TokenKind::RParen)
                && !self.is_eof()
            {
                self.index += 1;
            }
            if self.peek_kind() == Some(TokenKind::Var) {
                let name = self.expect_var()?;
                if self.consume(TokenKind::Equal) {
                    let expr = self.parse_expr()?;
                    params.insert(name, expr);
                } else {
                    params.insert(name, Expr::Undef);
                }
            } else {
                self.index += 1;
            }
            self.consume(TokenKind::Comma);
        }
        Ok(params)
    }

    /// Parse the `else`/`elsif` tail of an `if`/`unless` chain. `elsif C { … }`
    /// is desugared into `else { if C { … } [else { … }] }`.
    fn parse_else_chain(&mut self) -> Result<Vec<Stmt>> {
        if self.consume_keyword("elsif") {
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = self.parse_else_chain()?;
            return Ok(vec![Stmt::If { cond, then_body, else_body }]);
        }
        if self.consume_keyword("else") {
            return self.parse_block();
        }
        Ok(Vec::new())
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.consume(TokenKind::RBrace) && !self.is_eof() {
            if let Some(stmt) = self.parse_statement()? {
                stmts.push(stmt);
            } else {
                self.index += 1;
            }
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Option<Stmt>> {
        if self.consume_keyword("include") {
            self.consume_leading_namespace_prefix();
            let name = self.expect_ident()?;
            return Ok(Some(Stmt::Include(name)));
        }
        if self.consume_keyword("if") {
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = self.parse_else_chain()?;
            return Ok(Some(Stmt::If { cond, then_body, else_body }));
        }
        if self.consume_keyword("unless") {
            // `unless C { … } [else { … }]` is `if !C { … } else { … }`.
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = self.parse_else_chain()?;
            return Ok(Some(Stmt::If {
                cond: Cond::Not(Box::new(cond)),
                then_body,
                else_body,
            }));
        }
        if self.consume_keyword("case") {
            let expr = self.parse_expr()?;
            self.expect(TokenKind::LBrace)?;
            let mut branches = Vec::new();
            let mut default = Vec::new();
            while !self.consume(TokenKind::RBrace) && !self.is_eof() {
                if self.consume_keyword("default") {
                    self.consume(TokenKind::Colon);
                    default = self.parse_block()?;
                    continue;
                }
                let branch_expr = self.parse_expr()?;
                self.consume(TokenKind::Colon);
                let body = self.parse_block()?;
                branches.push((branch_expr, body));
            }
            return Ok(Some(Stmt::Case { expr, branches, default }));
        }
        if self.consume_keyword("fail") {
            if self.consume(TokenKind::LParen) {
                let message = if let Expr::String(msg) = self.parse_expr()? {
                    msg
                } else {
                    "fail".to_string()
                };
                self.consume(TokenKind::RParen);
                return Ok(Some(Stmt::Fail(message)));
            }
        }
        if self.peek_kind() == Some(TokenKind::Var) {
            let name = self.expect_var()?;
            if self.consume(TokenKind::Equal) {
                let expr = self.parse_value_expr()?;
                return Ok(Some(Stmt::VarAssign(name, expr)));
            }
        }
        // Accept a legacy leading `::` on resource-type references
        // (e.g. `::apache::vhost { ... }`). Modern Puppet drops the prefix;
        // we strip it and warn.
        let saved_index = self.index;
        let stripped_prefix = self.consume_leading_namespace_prefix();
        if self.peek_kind() == Some(TokenKind::Ident) {
            let rtype = self.expect_ident()?;
            if self.consume(TokenKind::LBrace) {
                let titles = self.parse_titles()?;
                let attrs = self.parse_attributes()?;
                self.consume(TokenKind::RBrace);
                return Ok(Some(Stmt::Resource { rtype, titles, attrs }));
            }
            // Resource collector: `Foo::Bar<| expr |>`. We don't model
            // virtual/exported-resource realisation, so just skip the body.
            if self.consume(TokenKind::LtPipe) {
                let _ = rtype;
                while !self.consume(TokenKind::PipeGt) && !self.is_eof() {
                    self.index += 1;
                }
                return Ok(Some(Stmt::Noop));
            }
        }
        if stripped_prefix {
            // The `::` didn't actually introduce a resource declaration — roll
            // the index back and drop the warning we queued so the caller can
            // try other statement shapes without spurious noise.
            self.index = saved_index;
            self.warnings.pop();
        }
        Ok(None)
    }

    fn parse_titles(&mut self) -> Result<Vec<Expr>> {
        let mut titles = Vec::new();
        while !self.consume(TokenKind::Colon) && !self.is_eof() {
            if self.peek_kind() == Some(TokenKind::Comma) {
                self.index += 1;
                continue;
            }
            titles.push(self.parse_expr()?);
        }
        Ok(titles)
    }

    fn parse_attributes(&mut self) -> Result<HashMap<String, Expr>> {
        let mut attrs = HashMap::new();
        while !self.peek_kind().map_or(false, |kind| kind == TokenKind::RBrace) && !self.is_eof()
        {
            if self.peek_kind() == Some(TokenKind::Comma) {
                self.index += 1;
                continue;
            }
            // `* => $hash` — splat-attributes merge. We don't model merging in
            // the evaluator, but we must accept the syntax so manifests using
            // it parse.
            let key = if self.consume(TokenKind::Star) {
                "*".to_string()
            } else {
                match self.parse_expr()? {
                    Expr::String(value) => value,
                    Expr::Var(var) => var.name,
                    expr => expr.as_string(),
                }
            };
            self.consume(TokenKind::FatArrow);
            let value = self.parse_value_expr()?;
            attrs.insert(key, value);
        }
        Ok(attrs)
    }

    /// Top-level: `or` (lowest precedence).
    fn parse_cond(&mut self) -> Result<Cond> {
        let mut left = self.parse_cond_and()?;
        while self.consume_keyword("or") {
            let right = self.parse_cond_and()?;
            left = Cond::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// Next level: `and`.
    fn parse_cond_and(&mut self) -> Result<Cond> {
        let mut left = self.parse_cond_unary()?;
        while self.consume_keyword("and") {
            let right = self.parse_cond_unary()?;
            left = Cond::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// Unary `!` and parenthesised compound conditions.
    fn parse_cond_unary(&mut self) -> Result<Cond> {
        if self.consume(TokenKind::LParen) {
            let cond = self.parse_cond()?;
            self.consume(TokenKind::RParen);
            return Ok(cond);
        }
        if self.consume(TokenKind::Bang) {
            return Ok(Cond::Not(Box::new(self.parse_cond_unary()?)));
        }
        self.parse_cond_atom()
    }

    /// Atomic condition: `defined(...)`, comparison (`==`/`!=`/`=~`/`!~`),
    /// `in` membership, or a bare expression treated as truthiness.
    fn parse_cond_atom(&mut self) -> Result<Cond> {
        if self.consume_keyword("defined") {
            self.consume(TokenKind::LParen);
            // `defined('$var')` / `defined($var)` — strict-vars guard.
            // We can't statically prove a variable is set during fixture-module
            // parsing, so treat this as always-false (which routes apt::pin into
            // its safe fallback branch). The point is to accept the syntax.
            if matches!(self.peek_kind(), Some(TokenKind::String) | Some(TokenKind::Var)) {
                self.index += 1;
                self.consume(TokenKind::RParen);
                return Ok(Cond::Compare {
                    left: Expr::Bool(false),
                    op: CompareOp::Eq,
                    right: Expr::Bool(true),
                });
            }
            let rtype = self.expect_ident()?;
            self.consume(TokenKind::LBracket);
            let title = match self.parse_expr()? {
                Expr::String(value) => value,
                expr => expr.as_string(),
            };
            self.consume(TokenKind::RBracket);
            self.consume(TokenKind::RParen);
            return Ok(Cond::Defined { rtype, title });
        }
        let left = self.parse_expr()?;
        if self.consume(TokenKind::EqEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::Eq, right });
        }
        if self.consume(TokenKind::NotEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::NotEq, right });
        }
        if self.consume(TokenKind::Match) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::Match, right });
        }
        if self.consume(TokenKind::NotMatch) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::NotMatch, right });
        }
        if self.consume(TokenKind::LtEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::LtEq, right });
        }
        if self.consume(TokenKind::GtEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::GtEq, right });
        }
        if self.consume(TokenKind::Lt) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::Lt, right });
        }
        if self.consume(TokenKind::Gt) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare { left, op: CompareOp::Gt, right });
        }
        if self.consume_keyword("in") {
            let right = self.parse_expr()?;
            return Ok(Cond::In { needle: left, haystack: right });
        }
        Ok(Cond::Truthy(left))
    }

    /// Parse an expression that may include boolean (`and`/`or`) and
    /// comparison (`==`, `!=`, `=~`, `!~`, `in`) operators — used in value
    /// positions like the RHS of a variable assignment. Falls through to a
    /// plain `parse_expr` when no boolean/comparison operator is present, so
    /// the resulting `Expr` tree stays minimal for simple cases.
    fn parse_value_expr(&mut self) -> Result<Expr> {
        let cond = self.parse_cond()?;
        Ok(match cond {
            Cond::Truthy(expr) => expr,
            other => Expr::Condition(Box::new(other)),
        })
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.consume(TokenKind::Dot) {
                let name = self.expect_ident()?;
                expr = Expr::MethodCall { target: Box::new(expr), name };
                continue;
            }
            let arith_op = match self.peek_kind() {
                Some(TokenKind::Plus) => Some(ArithOp::Add),
                Some(TokenKind::Minus) => Some(ArithOp::Sub),
                Some(TokenKind::Star) => Some(ArithOp::Mul),
                Some(TokenKind::Slash) => Some(ArithOp::Div),
                _ => None,
            };
            if let Some(op) = arith_op {
                self.index += 1;
                let right = self.parse_primary()?;
                expr = Expr::Arith { op, left: Box::new(expr), right: Box::new(right) };
                continue;
            }
            if self.consume(TokenKind::Question) {
                self.consume(TokenKind::LBrace);
                let mut cases = Vec::new();
                while !self.consume(TokenKind::RBrace) && !self.is_eof() {
                    if self.peek_kind() == Some(TokenKind::Comma) {
                        self.index += 1;
                        continue;
                    }
                    let key = self.parse_expr()?;
                    self.consume(TokenKind::FatArrow);
                    let value = self.parse_expr()?;
                    cases.push((key, value));
                }
                expr = Expr::Selector {
                    subject: Box::new(expr),
                    cases,
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        if self.consume(TokenKind::LParen) {
            let expr = self.parse_expr()?;
            self.consume(TokenKind::RParen);
            return Ok(expr);
        }
        if self.peek_kind() == Some(TokenKind::String) {
            let value = self.expect_string()?;
            return Ok(Expr::String(value));
        }
        if self.peek_kind() == Some(TokenKind::Regex) {
            let token = self.tokens[self.index].clone();
            self.index += 1;
            return Ok(Expr::Regex(token.text));
        }
        if self.peek_kind() == Some(TokenKind::Number) {
            let value = self.expect_number()?;
            return Ok(Expr::Integer(value));
        }
        if self.consume_keyword("true") {
            return Ok(Expr::Bool(true));
        }
        if self.consume_keyword("false") {
            return Ok(Expr::Bool(false));
        }
        if self.consume_keyword("undef") {
            return Ok(Expr::Undef);
        }
        if self.peek_kind() == Some(TokenKind::Var) {
            let var = self.expect_var()?;
            let mut path = Vec::new();
            while self.consume(TokenKind::LBracket) {
                let key = match self.parse_expr()? {
                    Expr::String(value) => value,
                    expr => expr.as_string(),
                };
                path.push(key);
                self.consume(TokenKind::RBracket);
            }
            return Ok(Expr::Var(VarRef { name: var, path }));
        }
        if self.peek_kind() == Some(TokenKind::Ident) {
            let ident = self.expect_ident()?;
            if self.consume(TokenKind::LParen) {
                let mut args = Vec::new();
                while !self.consume(TokenKind::RParen) && !self.is_eof() {
                    if self.peek_kind() == Some(TokenKind::Comma) {
                        self.index += 1;
                        continue;
                    }
                    args.push(self.parse_expr()?);
                }
                return Ok(Expr::FunctionCall { name: ident, args });
            }
            if self.allow_bare_vars {
                let mut path = Vec::new();
                while self.consume(TokenKind::LBracket) {
                    let key = match self.parse_expr()? {
                        Expr::String(value) => value,
                        expr => expr.as_string(),
                    };
                    path.push(key);
                    self.consume(TokenKind::RBracket);
                }
                return Ok(Expr::Var(VarRef { name: ident, path }));
            }
            if self.consume(TokenKind::LBracket) {
                let title = self.parse_expr()?;
                self.consume(TokenKind::RBracket);
                return Ok(Expr::ResourceRef {
                    rtype: ident,
                    title: Box::new(title),
                });
            }
            return Ok(Expr::String(ident));
        }
        if self.consume(TokenKind::LBracket) {
            let mut values = Vec::new();
            while !self.consume(TokenKind::RBracket) && !self.is_eof() {
                if self.peek_kind() == Some(TokenKind::Comma) {
                    self.index += 1;
                    continue;
                }
                values.push(self.parse_expr()?);
            }
            return Ok(Expr::Array(values));
        }
        if self.consume(TokenKind::LBrace) {
            let mut entries = Vec::new();
            while !self.consume(TokenKind::RBrace) && !self.is_eof() {
                if self.peek_kind() == Some(TokenKind::Comma) {
                    self.index += 1;
                    continue;
                }
                let key = self.parse_expr()?;
                self.consume(TokenKind::FatArrow);
                let value = self.parse_expr()?;
                entries.push((key, value));
            }
            return Ok(Expr::Hash(entries));
        }
        let token = self.peek_token();
        let detail = token
            .map(|tok| format!("{:?} '{}'", tok.kind, tok.text))
            .unwrap_or_else(|| "EOF".to_string());
        Err(anyhow::anyhow!("unexpected token {detail}"))
    }

    fn parse_inline_expr(source: &str) -> Result<Expr> {
        let mut parser = PuppetParser::new(source);
        parser.allow_bare_vars = true;
        parser.parse_expr()
    }

    fn expect_ident(&mut self) -> Result<String> {
        let token = self.expect_token(TokenKind::Ident)?;
        Ok(token.text.clone())
    }

    fn expect_var(&mut self) -> Result<String> {
        let token = self.expect_token(TokenKind::Var)?;
        Ok(token.text.trim_start_matches('$').to_string())
    }

    fn expect_string(&mut self) -> Result<String> {
        let token = self.expect_token(TokenKind::String)?;
        Ok(token.text.clone())
    }

    fn expect_number(&mut self) -> Result<i64> {
        let token = self.expect_token(TokenKind::Number)?;
        Ok(token.text.parse().unwrap_or_default())
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        self.expect_token(kind)?;
        Ok(())
    }

    fn expect_token(&mut self, kind: TokenKind) -> Result<&Token> {
        if self.peek_kind() == Some(kind) {
            let token = &self.tokens[self.index];
            self.index += 1;
            Ok(token)
        } else {
            Err(anyhow::anyhow!("expected token {:?}", kind))
        }
    }

    fn consume(&mut self, kind: TokenKind) -> bool {
        if self.peek_kind() == Some(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if let Some(token) = self.peek_token() {
            if token.kind == TokenKind::Ident && token.text == keyword {
                self.index += 1;
                return true;
            }
        }
        false
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.index).map(|token| token.kind)
    }

    fn peek_token(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn is_eof(&self) -> bool {
        self.index >= self.tokens.len()
    }
}

impl Expr {
    fn as_string(&self) -> String {
        match self {
            Expr::String(value) => value.clone(),
            Expr::Integer(value) => value.to_string(),
            Expr::Bool(value) => value.to_string(),
            Expr::Undef => "undef".to_string(),
            Expr::Array(_) => "array".to_string(),
            Expr::Hash(_) => "hash".to_string(),
            Expr::Var(var) => var.name.clone(),
            Expr::MethodCall { name, .. } => name.clone(),
            Expr::ResourceRef { rtype, .. } => rtype.clone(),
            Expr::FunctionCall { name, .. } => name.clone(),
            Expr::Regex(pattern) => pattern.clone(),
            Expr::Selector { .. } => "selector".to_string(),
            Expr::Condition(_) => "condition".to_string(),
            Expr::Arith { .. } => "arith".to_string(),
        }
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Ident,
    Var,
    String,
    Number,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Comma,
    Colon,
    Dot,
    Equal,
    FatArrow,
    EqEq,
    NotEq,
    Bang,
    Match,
    NotMatch,
    Regex,
    Question,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Plus,
    Minus,
    Star,
    Slash,
    /// `<|` — opening delimiter of a resource collector expression
    /// (e.g. `Apt::Key<| title == $title |>`).
    LtPipe,
    /// `|>` — closing delimiter of a resource collector expression.
    PipeGt,
    /// `|` — used as a parameter delimiter in lambda blocks and inside
    /// collector expressions.
    Pipe,
    /// `->` — ordering arrow between resources.
    Arrow,
    /// `~>` — notify arrow between resources.
    TildeArrow,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    text: String,
    offset: usize,
}

struct Lexer<'a> {
    input: &'a str,
    chars: Vec<char>,
    index: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            index: 0,
        }
    }

    fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.index += 1;
                continue;
            }
            if ch == '#' {
                self.consume_comment();
                continue;
            }
            let offset = self.index;
            match ch {
                '{' => {
                    tokens.push(self.make(TokenKind::LBrace, "{", offset));
                    self.index += 1;
                }
                '}' => {
                    tokens.push(self.make(TokenKind::RBrace, "}", offset));
                    self.index += 1;
                }
                '[' => {
                    tokens.push(self.make(TokenKind::LBracket, "[", offset));
                    self.index += 1;
                }
                ']' => {
                    tokens.push(self.make(TokenKind::RBracket, "]", offset));
                    self.index += 1;
                }
                '(' => {
                    tokens.push(self.make(TokenKind::LParen, "(", offset));
                    self.index += 1;
                }
                ')' => {
                    tokens.push(self.make(TokenKind::RParen, ")", offset));
                    self.index += 1;
                }
                ',' => {
                    tokens.push(self.make(TokenKind::Comma, ",", offset));
                    self.index += 1;
                }
                '?' => {
                    tokens.push(self.make(TokenKind::Question, "?", offset));
                    self.index += 1;
                }
                ':' => {
                    tokens.push(self.make(TokenKind::Colon, ":", offset));
                    self.index += 1;
                }
                '.' => {
                    tokens.push(self.make(TokenKind::Dot, ".", offset));
                    self.index += 1;
                }
                '=' => {
                    if self.peek_next() == Some('>') {
                        tokens.push(self.make(TokenKind::FatArrow, "=>", offset));
                        self.index += 2;
                    } else if self.peek_next() == Some('=') {
                        tokens.push(self.make(TokenKind::EqEq, "==", offset));
                        self.index += 2;
                    } else if self.peek_next() == Some('~') {
                        tokens.push(self.make(TokenKind::Match, "=~", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Equal, "=", offset));
                        self.index += 1;
                    }
                }
                '!' => {
                    if self.peek_next() == Some('=') {
                        tokens.push(self.make(TokenKind::NotEq, "!=", offset));
                        self.index += 2;
                    } else if self.peek_next() == Some('~') {
                        tokens.push(self.make(TokenKind::NotMatch, "!~", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Bang, "!", offset));
                        self.index += 1;
                    }
                }
                '<' => {
                    if self.peek_next() == Some('=') {
                        tokens.push(self.make(TokenKind::LtEq, "<=", offset));
                        self.index += 2;
                    } else if self.peek_next() == Some('|') {
                        tokens.push(self.make(TokenKind::LtPipe, "<|", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Lt, "<", offset));
                        self.index += 1;
                    }
                }
                '>' => {
                    if self.peek_next() == Some('=') {
                        tokens.push(self.make(TokenKind::GtEq, ">=", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Gt, ">", offset));
                        self.index += 1;
                    }
                }
                '/' if can_start_regex(tokens.last()) => {
                    let text = self.consume_regex();
                    tokens.push(self.make(TokenKind::Regex, &text, offset));
                }
                '/' => {
                    tokens.push(self.make(TokenKind::Slash, "/", offset));
                    self.index += 1;
                }
                '+' => {
                    tokens.push(self.make(TokenKind::Plus, "+", offset));
                    self.index += 1;
                }
                '-' => {
                    if self.peek_next() == Some('>') {
                        tokens.push(self.make(TokenKind::Arrow, "->", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Minus, "-", offset));
                        self.index += 1;
                    }
                }
                '~' => {
                    if self.peek_next() == Some('>') {
                        tokens.push(self.make(TokenKind::TildeArrow, "~>", offset));
                        self.index += 2;
                    } else {
                        // Unrecognized solo `~` — skip to preserve prior
                        // lexer behavior.
                        self.index += 1;
                    }
                }
                '|' => {
                    if self.peek_next() == Some('>') {
                        tokens.push(self.make(TokenKind::PipeGt, "|>", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Pipe, "|", offset));
                        self.index += 1;
                    }
                }
                '*' => {
                    tokens.push(self.make(TokenKind::Star, "*", offset));
                    self.index += 1;
                }
                '"' => {
                    let text = self.consume_string('"');
                    tokens.push(self.make(TokenKind::String, &text, offset));
                }
                '\'' => {
                    let text = self.consume_string('\'');
                    tokens.push(self.make(TokenKind::String, &text, offset));
                }
                '$' => {
                    let text = self.consume_variable();
                    tokens.push(self.make(TokenKind::Var, &text, offset));
                }
                _ => {
                    if ch.is_ascii_digit() {
                        let text = self.consume_number();
                        tokens.push(self.make(TokenKind::Number, &text, offset));
                    } else if is_ident_start(ch) {
                        let text = self.consume_ident();
                        tokens.push(self.make(TokenKind::Ident, &text, offset));
                    } else {
                        self.index += 1;
                    }
                }
            }
        }
        tokens
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn consume_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.index += 1;
            if ch == '\n' {
                break;
            }
        }
    }

    fn consume_string(&mut self, quote: char) -> String {
        // Skip opening quote.
        self.index += 1;
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch == quote {
                self.index += 1;
                break;
            }
            if ch == '\\' {
                if let Some(next) = self.peek_next() {
                    self.index += 2;
                    if quote == '"' {
                        match next {
                            'n' => out.push('\n'),
                            't' => out.push('\t'),
                            'r' => out.push('\r'),
                            's' => out.push(' '),
                            '0' => out.push('\0'),
                            '"' | '\\' | '$' => out.push(next),
                            other => {
                                out.push('\\');
                                out.push(other);
                            }
                        }
                    } else {
                        // Single-quoted: only '\\' and "\'" are escapes; otherwise the
                        // backslash is preserved literally.
                        match next {
                            '\\' | '\'' => out.push(next),
                            other => {
                                out.push('\\');
                                out.push(other);
                            }
                        }
                    }
                    continue;
                }
                // Trailing backslash at EOF — keep it and stop.
                out.push('\\');
                self.index += 1;
                break;
            }
            out.push(ch);
            self.index += 1;
        }
        out
    }

    fn consume_variable(&mut self) -> String {
        let start = self.index;
        self.index += 1;
        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                self.index += 1;
            } else if ch == ':' && self.peek_next() == Some(':') {
                self.index += 2;
            } else {
                break;
            }
        }
        self.input[start..self.index].to_string()
    }

    fn consume_ident(&mut self) -> String {
        let start = self.index;
        self.index += 1;
        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                self.index += 1;
            } else if ch == ':' && self.peek_next() == Some(':') {
                self.index += 2;
            } else {
                break;
            }
        }
        self.input[start..self.index].to_string()
    }

    /// Consume a `/pattern/[flags]` regex literal. Returns the pattern with
    /// any flags applied as Rust regex inline modifiers (e.g. `(?i)pattern`).
    fn consume_regex(&mut self) -> String {
        self.index += 1; // opening '/'
        let mut pattern = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\\' {
                // Preserve the escape sequence verbatim.
                pattern.push(ch);
                self.index += 1;
                if let Some(next) = self.peek() {
                    pattern.push(next);
                    self.index += 1;
                }
                continue;
            }
            if ch == '/' {
                self.index += 1;
                break;
            }
            if ch == '\n' {
                // Unterminated: bail out so we don't swallow the file.
                break;
            }
            pattern.push(ch);
            self.index += 1;
        }
        let mut flags = String::new();
        while let Some(ch) = self.peek() {
            if matches!(ch, 'i' | 'm' | 's' | 'x') {
                flags.push(ch);
                self.index += 1;
            } else {
                break;
            }
        }
        if flags.is_empty() {
            pattern
        } else {
            format!("(?{flags}){pattern}")
        }
    }

    fn consume_number(&mut self) -> String {
        let start = self.index;
        self.index += 1;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.index += 1;
            } else {
                break;
            }
        }
        self.input[start..self.index].to_string()
    }

    fn make(&self, kind: TokenKind, text: &str, offset: usize) -> Token {
        Token {
            kind,
            text: text.to_string(),
            offset,
        }
    }
}

/// Evaluate `<needle> in <haystack>`:
/// - haystack `Array` → element membership (value equality)
/// - haystack `Hash`  → key membership (needle stringified)
/// - haystack `String` → substring match
/// - everything else → false
fn eval_arith(left: &PuppetValue, op: ArithOp, right: &PuppetValue) -> PuppetValue {
    if let (PuppetValue::Integer(a), PuppetValue::Integer(b)) = (left, right) {
        let result = match op {
            ArithOp::Add => a.checked_add(*b),
            ArithOp::Sub => a.checked_sub(*b),
            ArithOp::Mul => a.checked_mul(*b),
            ArithOp::Div => (*b != 0).then(|| a / b),
        };
        if let Some(value) = result {
            return PuppetValue::Integer(value);
        }
    }
    // Hash-minus (`$h - 'key'`) and other non-integer combinations: keep the
    // left-hand value so downstream code still sees something sensible.
    left.clone()
}

fn eval_ordered_compare(left: &PuppetValue, op: CompareOp, right: &PuppetValue) -> bool {
    fn to_int(v: &PuppetValue) -> Option<i64> {
        match v {
            PuppetValue::Integer(n) => Some(*n),
            PuppetValue::String(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    }
    let ordering = match (to_int(left), to_int(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => left.as_string().cmp(&right.as_string()),
    };
    match op {
        CompareOp::Lt => ordering.is_lt(),
        CompareOp::Gt => ordering.is_gt(),
        CompareOp::LtEq => ordering.is_le(),
        CompareOp::GtEq => ordering.is_ge(),
        _ => false,
    }
}

fn eval_in(needle: &PuppetValue, haystack: &PuppetValue) -> bool {
    match haystack {
        PuppetValue::Array(items) => items.iter().any(|item| item == needle),
        PuppetValue::Hash(map) => map.contains_key(&needle.as_string()),
        PuppetValue::String(text) => text.contains(&needle.as_string()),
        _ => false,
    }
}

/// Evaluate `subject =~ pattern`. The pattern's `as_string()` is compiled
/// as a `regex::Regex` and matched (substring match — Puppet semantics).
fn eval_regex_match(subject: &PuppetValue, pattern: &PuppetValue) -> Result<bool> {
    let subject = subject.as_string();
    let pattern = pattern.as_string();
    let re = Regex::new(&pattern)
        .with_context(|| format!("invalid regex pattern: /{pattern}/"))?;
    Ok(re.is_match(&subject))
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// Heuristic to disambiguate `/` between regex-start and arithmetic.
/// Regent's parser doesn't currently model division, but we still want to
/// avoid surprising lexes — only treat `/` as a regex when the preceding
/// token leaves us in an "operand expected" position.
fn can_start_regex(prev: Option<&Token>) -> bool {
    let Some(prev) = prev else { return true };
    matches!(
        prev.kind,
        TokenKind::Match
            | TokenKind::NotMatch
            | TokenKind::EqEq
            | TokenKind::NotEq
            | TokenKind::Equal
            | TokenKind::FatArrow
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace
            // `}` ends a block and is followed by another `case` pattern in
            // `case` bodies, e.g. `} /^absent$/: { … }`.
            | TokenKind::RBrace
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Bang
    )
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn source_line_col(source: &str, byte_offset: usize) -> (usize, usize) {
    let clamped = byte_offset.min(source.len());
    let prefix = &source[..clamped];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let col = prefix.rsplit_once('\n').map(|(_, tail)| tail.len()).unwrap_or(prefix.len()) + 1;
    (line, col)
}

/// Minimal EPP template renderer. Supports:
///   * Plain text passthrough
///   * `<%= expr %>` output tags (variable / literal / boolean / integer / undef)
///   * `<% if cond { %> ... <% } else { %> ... <% } %>` conditionals
///     (and `} elsif cond { %>` chains)
///   * Optional `-` markers (`<%-`, `-%>`) trimming surrounding whitespace
///   * Skips the `<%- | params | -%>` parameter header
///
/// Anything richer (function calls, arithmetic, iteration) renders the literal
/// text of the surrounding template — close enough for content-regex matchers
/// in unit tests.
fn render_epp(template: &str, params: &PuppetValue) -> String {
    let tokens = tokenize_epp(template);
    let params_map: HashMap<String, PuppetValue> = match params {
        PuppetValue::Hash(m) => m.clone(),
        _ => HashMap::new(),
    };
    let mut out = String::new();
    // Each entry is (emitting?, branch_already_taken_in_this_if?). The branch
    // flag is only meaningful for `if`/`elsif`/`else` chains.
    let mut stack: Vec<(bool, bool)> = Vec::new();
    let emitting = |s: &[(bool, bool)]| s.iter().all(|(e, _)| *e);

    for tok in tokens {
        match tok {
            EppToken::Text(text) => {
                if emitting(&stack) {
                    out.push_str(&text);
                }
            }
            EppToken::Output(expr) => {
                if emitting(&stack) {
                    out.push_str(&epp_eval(expr.trim(), &params_map).as_string());
                }
            }
            EppToken::Code(code) => {
                let trimmed = code.trim();
                if trimmed.starts_with('|') {
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("if ") {
                    let cond_expr = rest.trim_end_matches('{').trim();
                    let parent = emitting(&stack);
                    let cond = parent && epp_truthy(&epp_eval(cond_expr, &params_map));
                    stack.push((cond, cond));
                    continue;
                }
                if trimmed.starts_with("unless ") {
                    let cond_expr = trimmed["unless ".len()..].trim_end_matches('{').trim();
                    let parent = emitting(&stack);
                    let cond = parent && !epp_truthy(&epp_eval(cond_expr, &params_map));
                    stack.push((cond, cond));
                    continue;
                }
                if trimmed == "} else {" || trimmed.starts_with("} else") {
                    if let Some((_, taken)) = stack.pop() {
                        let parent = emitting(&stack);
                        let next = parent && !taken;
                        stack.push((next, taken || next));
                    }
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("} elsif ") {
                    if let Some((_, taken)) = stack.pop() {
                        let cond_src = rest.trim_end_matches('{').trim();
                        let parent = emitting(&stack);
                        let next = parent && !taken && epp_truthy(&epp_eval(cond_src, &params_map));
                        stack.push((next, taken || next));
                    }
                    continue;
                }
                if trimmed == "}" {
                    stack.pop();
                    continue;
                }
                // Unknown code tag — ignore (no error, mirrors tolerant style
                // used elsewhere in the evaluator).
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
enum EppToken {
    Text(String),
    Output(String),
    Code(String),
}

fn tokenize_epp(template: &str) -> Vec<EppToken> {
    let chars: Vec<char> = template.chars().collect();
    let mut tokens = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' && chars.get(i + 1) == Some(&'%') {
            let mut start = i + 2;
            let trim_left = chars.get(start) == Some(&'-');
            if trim_left {
                start += 1;
            }
            let is_output = chars.get(start) == Some(&'=');
            if is_output {
                start += 1;
            }
            // Find closing %> (handle -%>)
            let mut end = start;
            let mut trim_right = false;
            let mut found = false;
            while end < chars.len() {
                if chars[end] == '-'
                    && chars.get(end + 1) == Some(&'%')
                    && chars.get(end + 2) == Some(&'>')
                {
                    trim_right = true;
                    found = true;
                    break;
                }
                if chars[end] == '%' && chars.get(end + 1) == Some(&'>') {
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                // Unterminated tag — bail out and emit as literal text.
                text.extend(chars[i..].iter());
                break;
            }
            if trim_left {
                while let Some(c) = text.chars().last() {
                    if c == ' ' || c == '\t' {
                        text.pop();
                    } else {
                        break;
                    }
                }
            }
            if !text.is_empty() {
                tokens.push(EppToken::Text(std::mem::take(&mut text)));
            }
            let content: String = chars[start..end].iter().collect();
            if is_output {
                tokens.push(EppToken::Output(content));
            } else {
                tokens.push(EppToken::Code(content));
            }
            i = if trim_right { end + 3 } else { end + 2 };
            if trim_right {
                while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '\n' {
                    i += 1;
                }
            }
            continue;
        }
        text.push(chars[i]);
        i += 1;
    }
    if !text.is_empty() {
        tokens.push(EppToken::Text(text));
    }
    tokens
}

fn epp_eval(source: &str, params: &HashMap<String, PuppetValue>) -> PuppetValue {
    let s = source.trim();
    if let Some(name) = s.strip_prefix('$') {
        return params.get(name).cloned().unwrap_or(PuppetValue::Undef);
    }
    if s.len() >= 2 {
        if s.starts_with('"') && s.ends_with('"') {
            return PuppetValue::String(s[1..s.len() - 1].to_string());
        }
        if s.starts_with('\'') && s.ends_with('\'') {
            return PuppetValue::String(s[1..s.len() - 1].to_string());
        }
    }
    if let Ok(n) = s.parse::<i64>() {
        return PuppetValue::Integer(n);
    }
    match s {
        "true" => PuppetValue::Bool(true),
        "false" => PuppetValue::Bool(false),
        "undef" => PuppetValue::Undef,
        _ => PuppetValue::Undef,
    }
}

fn epp_truthy(value: &PuppetValue) -> bool {
    match value {
        PuppetValue::Bool(b) => *b,
        PuppetValue::Undef => false,
        PuppetValue::String(s) => !s.is_empty(),
        PuppetValue::Array(a) => !a.is_empty(),
        PuppetValue::Hash(h) => !h.is_empty(),
        PuppetValue::Integer(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_module(name: &str, manifest: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        fs::create_dir_all(&manifests).unwrap();
        fs::write(manifests.join(format!("{name}.pp")), manifest).unwrap();
        dir
    }

    #[test]
    fn parser_accepts_resource_collector_chain_and_case_regex_after_brace() {
        // Exercises the constructs that previously caused apt/manifests/key.pp
        // to be skipped: chain operators between resources, resource
        // collectors, and a case branch whose regex pattern follows `}`.
        let mut parser = PuppetParser::new(
            r#"
class foo {
  case $bar {
    /^a$/: {
      thing { 'x': } -> anchor { 'a': }
      thing { 'y': } ~> anchor { 'b': }
      Apt::Key<| title == $title |>
    }
    /^b$/: {}
    default: {}
  }
}
"#,
        );
        let defs = parser.parse_definitions().expect("parse");
        assert_eq!(defs.len(), 1);
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::Case { branches, .. }] => {
                    assert_eq!(branches.len(), 2, "expected two regex branches");
                    let first_body = &branches[0].1;
                    // Two `Resource` declarations + `Noop` collector.
                    assert!(matches!(first_body[0], Stmt::Resource { .. }));
                    assert!(matches!(first_body[1], Stmt::Resource { .. }));
                    assert!(matches!(first_body[2], Stmt::Resource { .. }));
                    assert!(matches!(first_body[3], Stmt::Resource { .. }));
                    assert!(matches!(first_body[4], Stmt::Noop));
                }
                other => panic!("unexpected body: {other:?}"),
            },
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn parser_accepts_legacy_leading_namespace_prefix_in_include() {
        let mut parser = PuppetParser::new(
            "class foo {\n  include ::stdlib\n}\n",
        );
        let defs = parser.parse_definitions().expect("parse");
        assert_eq!(defs.len(), 1);
        let warnings: Vec<String> = parser.warnings.iter().map(|w| w.message.clone()).collect();
        assert_eq!(warnings.len(), 1, "expected one deprecation warning");
        assert!(warnings[0].contains("leading `::`"));
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::Include(name)] => assert_eq!(name, "stdlib"),
                other => panic!("unexpected body: {other:?}"),
            },
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn parser_accepts_legacy_leading_namespace_prefix_in_inherits() {
        let mut parser = PuppetParser::new("class child inherits ::base {}\n");
        let defs = parser.parse_definitions().expect("parse");
        assert_eq!(defs.len(), 1);
        assert_eq!(parser.warnings.len(), 1);
        match &defs[0] {
            PuppetDef::Class(c) => assert_eq!(c.parent.as_deref(), Some("base")),
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn parser_accepts_legacy_leading_namespace_prefix_on_resource_type() {
        let mut parser = PuppetParser::new(
            "class foo {\n  ::apache::vhost { 'site': port => 80 }\n}\n",
        );
        let defs = parser.parse_definitions().expect("parse");
        assert_eq!(parser.warnings.len(), 1);
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::Resource { rtype, .. }] => assert_eq!(rtype, "apache::vhost"),
                other => panic!("unexpected body: {other:?}"),
            },
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn parser_does_not_warn_when_no_leading_prefix_present() {
        let mut parser = PuppetParser::new(
            "class foo {\n  include stdlib\n}\n",
        );
        let _ = parser.parse_definitions().expect("parse");
        assert!(parser.warnings.is_empty());
    }

    #[test]
    fn double_quoted_string_handles_escaped_quote_and_braces() {
        // Escaped quotes inside `"..."` must not terminate the string early —
        // otherwise literal `}` characters inside an embedded JSON blob (e.g.
        // a `concat::fragment` content) tokenize as RBrace and break parsing.
        let manifest = r#"
            class foo {
              $payload = "{\"k\": \"v\", \"nested\": {\"a\": 1}}"
              notify { $payload: }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(HashMap::new()),
            )
            .expect("manifest with escaped quotes and embedded braces must parse");
        assert!(catalog.contains("notify", "{\"k\": \"v\", \"nested\": {\"a\": 1}}"));
    }

    #[test]
    fn function_call_with_trailing_comma_in_resource_attrs() {
        let manifest = r#"
            class foo {
              file { '/x':
                content => template('mod/t.erb'),
                mode    => '0644',
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class("foo", &PuppetValue::Hash(HashMap::new()), &PuppetValue::Hash(HashMap::new()))
            .expect("class with template(...) followed by trailing comma must parse");
        let resource = catalog.find("file", "/x").expect("file resource present");
        assert_eq!(
            resource.attributes.get("mode"),
            Some(&PuppetValue::String("0644".to_string()))
        );
    }

    #[test]
    fn regex_match_inside_if_includes_resource() {
        // `=~` against a regex literal must parse and evaluate to true,
        // including the resource declared in the matching branch.
        let manifest = r#"
            class foo {
              if $::osfamily =~ /^(Red ?Hat|Cent|Fedora)/ {
                file { '/etc/rhel-only': ensure => file }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = HashMap::new();
        facts.insert("osfamily".to_string(), PuppetValue::String("RedHat".to_string()));
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("manifest with =~ /.../ must parse");
        assert!(catalog.contains("file", "/etc/rhel-only"));
    }

    #[test]
    fn regex_non_match_inside_if_excludes_resource() {
        let manifest = r#"
            class foo {
              if $::osfamily !~ /Debian/ {
                file { '/etc/non-debian': ensure => file }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = HashMap::new();
        facts.insert("osfamily".to_string(), PuppetValue::String("RedHat".to_string()));
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("manifest with !~ /.../ must parse");
        assert!(catalog.contains("file", "/etc/non-debian"));
    }

    #[test]
    fn regex_with_case_insensitive_flag() {
        let manifest = r#"
            class foo {
              if $::osfamily =~ /redhat/i {
                file { '/etc/match': ensure => file }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = HashMap::new();
        facts.insert("osfamily".to_string(), PuppetValue::String("RedHat".to_string()));
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("/.../i flag must compile");
        assert!(catalog.contains("file", "/etc/match"));
    }

    #[test]
    fn regex_non_match_when_subject_matches_excludes_resource() {
        // Confirms !~ returns false when the pattern actually matches.
        let manifest = r#"
            class foo {
              if $::osfamily !~ /Red ?Hat/ {
                file { '/etc/should-not-exist': ensure => file }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = HashMap::new();
        facts.insert("osfamily".to_string(), PuppetValue::String("RedHat".to_string()));
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("manifest must parse");
        assert!(!catalog.contains("file", "/etc/should-not-exist"));
    }

    fn eval_with_osfamily(manifest: &str, osfamily: &str) -> PuppetCatalog {
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = HashMap::new();
        facts.insert("osfamily".to_string(), PuppetValue::String(osfamily.to_string()));
        evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(HashMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("manifest must parse")
    }

    #[test]
    fn unless_with_truthy_condition_skips_body() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  unless $::osfamily == 'RedHat' {
                    file { '/etc/non-rhel': ensure => file }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(!catalog.contains("file", "/etc/non-rhel"));
    }

    #[test]
    fn unless_with_falsy_condition_runs_body() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  unless $::osfamily == 'Debian' {
                    file { '/etc/not-debian': ensure => file }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/not-debian"));
    }

    #[test]
    fn and_or_short_circuit_compose() {
        // `(osfamily == RedHat) and ($::osfamily =~ /Red/)` → true
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  if $::osfamily == 'RedHat' and $::osfamily =~ /Red/ {
                    file { '/etc/and-true': ensure => file }
                  }
                  if $::osfamily == 'Debian' or $::osfamily =~ /Hat/ {
                    file { '/etc/or-true': ensure => file }
                  }
                  if $::osfamily == 'Debian' or $::osfamily == 'Suse' {
                    file { '/etc/or-false': ensure => file }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/and-true"));
        assert!(catalog.contains("file", "/etc/or-true"));
        assert!(!catalog.contains("file", "/etc/or-false"));
    }

    #[test]
    fn assignment_with_boolean_and_comparison_chain() {
        // RHS of `$x = ...` may use `and`/`or` and comparisons. Previously the
        // parser only accepted bare expressions here, so multi-clause gates
        // had to be rewritten as `if`/`else` blocks.
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  $is_red = $::osfamily == 'RedHat' and $::osfamily =~ /Red/
                  $is_other = $::osfamily == 'Debian' or $::osfamily == 'Suse'
                  if $is_red {
                    file { '/etc/red': ensure => file }
                  }
                  if $is_other {
                    file { '/etc/other': ensure => file }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/red"));
        assert!(!catalog.contains("file", "/etc/other"));
    }

    #[test]
    fn case_with_regex_branch_matches() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  case $::osfamily {
                    /^Red/: { file { '/etc/case-regex': ensure => file } }
                    default: { file { '/etc/case-default': ensure => file } }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/case-regex"));
        assert!(!catalog.contains("file", "/etc/case-default"));
    }

    #[test]
    fn case_with_array_pattern_matches_any_member() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  case $::osfamily {
                    ['RedHat', 'CentOS', 'Fedora']: { file { '/etc/rhel-like': ensure => file } }
                    ['Debian', 'Ubuntu']:           { file { '/etc/debian-like': ensure => file } }
                    default:                         { file { '/etc/other': ensure => file } }
                  }
                }
            "#,
            "Fedora",
        );
        assert!(catalog.contains("file", "/etc/rhel-like"));
        assert!(!catalog.contains("file", "/etc/debian-like"));
        assert!(!catalog.contains("file", "/etc/other"));
    }

    #[test]
    fn case_falls_through_to_default_when_no_branch_matches() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  case $::osfamily {
                    /Debian/: { file { '/etc/deb': ensure => file } }
                    default:  { file { '/etc/default-branch': ensure => file } }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/default-branch"));
        assert!(!catalog.contains("file", "/etc/deb"));
    }

    #[test]
    fn in_operator_against_array() {
        let manifest = r#"
            class foo {
              $oses = ['RedHat', 'CentOS', 'Fedora']
              if $::osfamily in $oses {
                file { '/etc/in-array': ensure => file }
              }
              if 'Debian' in $oses {
                file { '/etc/in-debian': ensure => file }
              }
            }
        "#;
        let catalog = eval_with_osfamily(manifest, "RedHat");
        assert!(catalog.contains("file", "/etc/in-array"));
        assert!(!catalog.contains("file", "/etc/in-debian"));
    }

    #[test]
    fn in_operator_against_string() {
        let catalog = eval_with_osfamily(
            r#"
                class foo {
                  if 'Hat' in $::osfamily {
                    file { '/etc/substring-match': ensure => file }
                  }
                }
            "#,
            "RedHat",
        );
        assert!(catalog.contains("file", "/etc/substring-match"));
    }

    #[test]
    fn loader_picks_up_sibling_manifest_files() {
        // Puppet's manifest autoloader resolves `mod::foo` to `manifests/foo.pp`.
        // The loader must read every .pp in the manifests dir, not just init.pp.
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::write(manifests.join("init.pp"), "class mymod { include mymod::foo }").unwrap();
        std::fs::write(
            manifests.join("foo.pp"),
            "class mymod::foo { file { '/etc/sibling-loaded': ensure => file } }",
        )
        .unwrap();

        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        assert!(
            evaluator.class_names().iter().any(|n| n == "mymod::foo"),
            "sibling manifests/foo.pp must be autoloaded, got: {:?}",
            evaluator.class_names()
        );
        let catalog = evaluator
            .evaluate_class("mymod", &PuppetValue::Hash(HashMap::new()), &PuppetValue::Hash(HashMap::new()))
            .expect("evaluating mymod (which includes mymod::foo) must succeed");
        assert!(catalog.contains("file", "/etc/sibling-loaded"));
    }

    #[test]
    fn loader_picks_up_nested_manifest_files() {
        // `mod::sub::leaf` should autoload from manifests/sub/leaf.pp.
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        std::fs::create_dir_all(manifests.join("sub")).unwrap();
        std::fs::write(manifests.join("init.pp"), "class mymod { include mymod::sub::leaf }").unwrap();
        std::fs::write(
            manifests.join("sub").join("leaf.pp"),
            "class mymod::sub::leaf { file { '/etc/nested-loaded': ensure => file } }",
        )
        .unwrap();

        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        assert!(
            evaluator.class_names().iter().any(|n| n == "mymod::sub::leaf"),
            "nested manifests/sub/leaf.pp must be autoloaded, got: {:?}",
            evaluator.class_names()
        );
    }

    #[test]
    fn epp_call_with_trailing_comma() {
        let manifest = r#"
            class foo {
              file { '/y':
                content => epp('mod/t.epp', { 'k' => 'v' }),
                ensure  => file,
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class("foo", &PuppetValue::Hash(HashMap::new()), &PuppetValue::Hash(HashMap::new()))
            .expect("class with epp(...) followed by trailing comma must parse");
        assert!(catalog.contains("file", "/y"));
    }
}
