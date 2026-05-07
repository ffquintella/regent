use anyhow::{Context, Result};
use std::collections::HashSet;
use std::collections::HashMap;
use std::path::Path;

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

impl PuppetEvaluator {
    pub fn new(module_path: &Path) -> Result<Self> {
        let fixtures = discover_fixture_module_paths(module_path);
        let module = PuppetModule::load_with_fixtures(module_path, &fixtures)?;
        Ok(Self { module })
    }

    pub fn is_define(&self, name: &str) -> bool {
        self.module.defines.contains_key(name)
    }

    pub fn evaluate_class(
        &self,
        name: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<PuppetCatalog> {
        let mut ctx = EvalContext::new(facts.clone(), params.clone(), &self.module);
        ctx.evaluate_class(name)?;
        Ok(ctx.catalog)
    }

    pub fn evaluate_define(
        &self,
        name: &str,
        title: &str,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<PuppetCatalog> {
        let mut ctx = EvalContext::new(facts.clone(), params.clone(), &self.module);
        ctx.evaluate_define(name, title)?;
        Ok(ctx.catalog)
    }
}

#[derive(Debug, Clone)]
struct PuppetModule {
    classes: HashMap<String, ClassDef>,
    defines: HashMap<String, DefineDef>,
}

impl PuppetModule {
    fn load_with_fixtures(module_path: &Path, fixture_module_paths: &[std::path::PathBuf]) -> Result<Self> {
        let mut classes = HashMap::new();
        let mut defines = HashMap::new();

        // Load fixtures first so the primary module's defs win on conflict.
        for fixture_path in fixture_module_paths {
            // Be tolerant: a single broken fixture should not block the run.
            if let Err(err) = load_manifests_into(fixture_path, &mut classes, &mut defines) {
                eprintln!(
                    "warning: skipping fixture module {}: {}",
                    fixture_path.display(),
                    err
                );
            }
        }

        load_manifests_into(module_path, &mut classes, &mut defines)
            .with_context(|| format!("load manifests for {}", module_path.display()))?;

        Ok(Self { classes, defines })
    }
}

fn load_manifests_into(
    module_path: &Path,
    classes: &mut HashMap<String, ClassDef>,
    defines: &mut HashMap<String, DefineDef>,
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
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("read manifest {}", path.display()))?;
            let mut parser = PuppetParser::new(&content);
            let defs = parser.parse_definitions().with_context(|| {
                format!("parse manifest {}:{}", path.display(), parser.position())
            })?;
            for def in defs {
                match def {
                    PuppetDef::Class(def) => {
                        classes.insert(def.name.clone(), def);
                    }
                    PuppetDef::Define(def) => {
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
}

#[derive(Debug, Clone)]
struct DefineDef {
    name: String,
    params: HashMap<String, Expr>,
    body: Vec<Stmt>,
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
}

#[derive(Debug, Clone)]
enum Cond {
    Not(Box<Cond>),
    Defined { rtype: String, title: String },
    Compare { left: Expr, op: CompareOp, right: Expr },
    Truthy(Expr),
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    NotEq,
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

    fn evaluate_define(&mut self, name: &str, title: &str) -> Result<()> {
        let define_def = self
            .module
            .defines
            .get(name)
            .with_context(|| format!("define {name} not found"))?
            .clone();
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
                        let branch_value = self.eval_expr(branch_expr)?;
                        if branch_value == value {
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
            }
        }
        Ok(())
    }

    fn eval_cond(&mut self, cond: &Cond) -> Result<bool> {
        Ok(match cond {
            Cond::Not(inner) => !self.eval_cond(inner)?,
            Cond::Defined { rtype, title } => {
                let rtype = normalize_rtype(rtype);
                self.catalog.contains(&rtype, title)
            }
            Cond::Compare { left, op, right } => {
                let left = self.eval_expr(left)?;
                let right = self.eval_expr(right)?;
                match op {
                    CompareOp::Eq => left == right,
                    CompareOp::NotEq => left != right,
                }
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
                    "template" | "epp" | "inline_template" | "inline_epp" => {
                        let source = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        PuppetValue::String(format!("<{name}:{source}>"))
                    }
                    _ => PuppetValue::Undef,
                }
            }
        })
    }

    fn resolve_var(&self, var: &VarRef) -> PuppetValue {
        let mut value = if var.name == "facts" {
            self.facts.clone()
        } else {
            self.vars
                .get(&var.name)
                .cloned()
                .unwrap_or(PuppetValue::Undef)
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
}

impl<'a> PuppetParser<'a> {
    fn new(source: &'a str) -> Self {
        let tokens = Lexer::new(source).tokenize();
        Self {
            source,
            tokens,
            index: 0,
            allow_bare_vars: false,
        }
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
            Some(self.expect_ident()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(ClassDef { name, params, parent, body })
    }

    fn parse_define_def(&mut self) -> Result<DefineDef> {
        let name = self.expect_ident()?;
        let params = self.parse_param_list()?;
        let body = self.parse_block()?;
        Ok(DefineDef { name, params, body })
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
            let name = self.expect_ident()?;
            return Ok(Some(Stmt::Include(name)));
        }
        if self.consume_keyword("if") {
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = if self.consume_keyword("else") {
                self.parse_block()?
            } else {
                Vec::new()
            };
            return Ok(Some(Stmt::If { cond, then_body, else_body }));
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
                let expr = self.parse_expr()?;
                return Ok(Some(Stmt::VarAssign(name, expr)));
            }
        }
        if self.peek_kind() == Some(TokenKind::Ident) {
            let rtype = self.expect_ident()?;
            if self.consume(TokenKind::LBrace) {
                let titles = self.parse_titles()?;
                let attrs = self.parse_attributes()?;
                self.consume(TokenKind::RBrace);
                return Ok(Some(Stmt::Resource { rtype, titles, attrs }));
            }
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
            let key = match self.parse_expr()? {
                Expr::String(value) => value,
                Expr::Var(var) => var.name,
                expr => expr.as_string(),
            };
            self.consume(TokenKind::FatArrow);
            let value = self.parse_expr()?;
            attrs.insert(key, value);
        }
        Ok(attrs)
    }

    fn parse_cond(&mut self) -> Result<Cond> {
        if self.consume(TokenKind::LParen) {
            let cond = self.parse_cond()?;
            self.consume(TokenKind::RParen);
            return Ok(cond);
        }
        if self.consume(TokenKind::Bang) {
            return Ok(Cond::Not(Box::new(self.parse_cond()?)));
        }
        if self.consume_keyword("defined") {
            self.consume(TokenKind::LParen);
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
        Ok(Cond::Truthy(left))
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.consume(TokenKind::Dot) {
                let name = self.expect_ident()?;
                expr = Expr::MethodCall { target: Box::new(expr), name };
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
                    } else {
                        tokens.push(self.make(TokenKind::Equal, "=", offset));
                        self.index += 1;
                    }
                }
                '!' => {
                    if self.peek_next() == Some('=') {
                        tokens.push(self.make(TokenKind::NotEq, "!=", offset));
                        self.index += 2;
                    } else {
                        tokens.push(self.make(TokenKind::Bang, "!", offset));
                        self.index += 1;
                    }
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
        self.index += 1;
        let start = self.index;
        while let Some(ch) = self.peek() {
            self.index += 1;
            if ch == quote {
                break;
            }
        }
        self.input[start..self.index - 1].to_string()
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

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
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
