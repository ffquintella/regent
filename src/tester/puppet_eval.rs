use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use indexmap::IndexMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum PuppetValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<PuppetValue>),
    // Insertion-ordered so rendered templates / catalogs reproduce the key
    // order Puppet preserves (e.g. an sshd_config built from an ordered data
    // hash). Requires serde_json's `preserve_order` feature for the JSON→value
    // path to keep order too.
    Hash(IndexMap<String, PuppetValue>),
    Undef,
}

impl PuppetValue {
    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => PuppetValue::Undef,
            serde_json::Value::Bool(value) => PuppetValue::Bool(*value),
            serde_json::Value::Number(value) => match value.as_i64() {
                Some(i) => PuppetValue::Integer(i),
                None => PuppetValue::Float(value.as_f64().unwrap_or_default()),
            },
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
            return PuppetValue::Hash(IndexMap::new());
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
            PuppetValue::Float(value) => format_puppet_float(*value),
            PuppetValue::Bool(value) => value.to_string(),
            PuppetValue::Array(values) => {
                // Puppet stringifies an array with bracket delimiters and formats
                // each *contained* value with `%p` (the programmatic form), which
                // double-quotes string members. So `['a', 'b']` renders as
                // `["a", "b"]`, not the bare `[a, b]` the elements' own
                // `as_string` would give.
                let items = values
                    .iter()
                    .map(|value| value.as_programmatic_string())
                    .collect::<Vec<_>>();
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

    /// Puppet's `%p` ("programmatic") string form, used for values *contained*
    /// in an array (or hash). It differs from [`as_string`] only for strings,
    /// which are double-quoted (with `"` and `\` escaped); every other value
    /// type — including nested arrays/hashes, which carry their own delimiters —
    /// is already self-delimiting and reuses [`as_string`].
    fn as_programmatic_string(&self) -> String {
        match self {
            PuppetValue::String(value) => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{escaped}\"")
            }
            other => other.as_string(),
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
            PuppetValue::Float(_) => true,
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
    pub attributes: IndexMap<String, PuppetValue>,
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
        self.resources
            .get(resource_type)
            .and_then(|map| map.get(title))
    }

    /// Every resource in the catalog, in unspecified order. Used by
    /// relationship checks that must scan the whole graph.
    pub fn iter_resources(&self) -> impl Iterator<Item = &PuppetResource> {
        self.resources
            .values()
            .flat_map(|by_title| by_title.values())
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

    pub fn is_class(&self, name: &str) -> bool {
        self.module.classes.contains_key(name)
    }

    /// Whether `type_name` (a `types/` alias such as `Ssh::Yes_no` or a built-in
    /// data type) admits `value`. Returns `None` when the name is neither a
    /// known alias nor a built-in data type, so the caller can report the
    /// missing type rather than silently passing. Powers `allow_value`.
    pub fn type_allows(&self, type_name: &str, value: &PuppetValue) -> Option<bool> {
        if !self.module.type_aliases.contains_key(type_name) && !is_data_type(type_name) {
            return None;
        }
        let spec = TypeSpec {
            name: type_name.to_string(),
            args: Vec::new(),
        };
        Some(eval_type_match(value, &spec, &self.module.type_aliases))
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
        self.evaluate_class_traced(name, facts, params)
            .map(|(c, _)| c)
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
        ctx.subject_class = Some(name.to_string());
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
    /// Puppet type aliases (`type Foo::Bar = Enum[...]`) discovered under the
    /// `types/` tree of the primary module and its fixtures, keyed by their
    /// exact written name. Used for `allow_value` matching and parameter type
    /// validation.
    type_aliases: HashMap<String, TypeSpec>,
    /// Module Hiera data (hiera.yaml + data/*.yaml) for automatic class
    /// parameter lookup.
    hiera: Hiera,
}

/// Module-level Hiera (data-in-modules): the ordered, fact-interpolated path
/// templates from `hiera.yaml` plus every loaded `data/**/*.yaml` file. Drives
/// automatic class-parameter lookup (`<class>::<param>`).
#[derive(Default, Debug, Clone)]
struct Hiera {
    /// Ordered path templates relative to the datadir, each possibly containing
    /// `%{facts.x.y}` placeholders.
    paths: Vec<String>,
    /// Loaded data keyed by relative path (e.g. `os/RedHat/9.yaml`).
    data: HashMap<String, HashMap<String, PuppetValue>>,
}

impl Hiera {
    fn load(module_path: &Path) -> Hiera {
        let mut paths = Vec::new();
        let mut datadir = "data".to_string();
        if let Ok(content) = std::fs::read_to_string(module_path.join("hiera.yaml")) {
            if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(d) = yaml
                    .get("defaults")
                    .and_then(|d| d.get("datadir"))
                    .and_then(|v| v.as_str())
                {
                    datadir = d.to_string();
                }
                if let Some(h) = yaml.get("hierarchy").and_then(|v| v.as_sequence()) {
                    for entry in h {
                        if let Some(ps) = entry.get("paths").and_then(|v| v.as_sequence()) {
                            for p in ps {
                                if let Some(s) = p.as_str() {
                                    paths.push(s.to_string());
                                }
                            }
                        } else if let Some(p) = entry.get("path").and_then(|v| v.as_str()) {
                            paths.push(p.to_string());
                        }
                    }
                }
            }
        }

        let mut data = HashMap::new();
        let datadir_path = module_path.join(&datadir);
        let mut stack = vec![datadir_path.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                    continue;
                }
                let Ok(rel) = path.strip_prefix(&datadir_path) else {
                    continue;
                };
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Ok(serde_yaml::Value::Mapping(map)) =
                    serde_yaml::from_str::<serde_yaml::Value>(&content)
                {
                    let mut m = HashMap::new();
                    for (k, v) in map {
                        if let Some(ks) = k.as_str() {
                            m.insert(ks.to_string(), yaml_to_puppet(&v));
                        }
                    }
                    data.insert(rel_str, m);
                }
            }
        }
        Hiera { paths, data }
    }

    /// Look up `key` (e.g. `ssh::server::include`) across the fact-interpolated
    /// hierarchy, first match wins. A path whose placeholders can't all be
    /// resolved from `facts` is skipped (as Hiera does).
    fn lookup(&self, key: &str, facts: &PuppetValue) -> Option<PuppetValue> {
        for template in &self.paths {
            let Some(rel) = hiera_interpolate(template, facts) else {
                continue;
            };
            if let Some(value) = self.data.get(&rel).and_then(|m| m.get(key)) {
                return Some(value.clone());
            }
        }
        None
    }
}

/// Convert a YAML value to a [`PuppetValue`] (null → Undef).
fn yaml_to_puppet(value: &serde_yaml::Value) -> PuppetValue {
    match value {
        serde_yaml::Value::Null => PuppetValue::Undef,
        serde_yaml::Value::Bool(b) => PuppetValue::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PuppetValue::Integer(i)
            } else {
                PuppetValue::String(n.to_string())
            }
        }
        serde_yaml::Value::String(s) => PuppetValue::String(s.clone()),
        serde_yaml::Value::Sequence(items) => {
            PuppetValue::Array(items.iter().map(yaml_to_puppet).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut m = IndexMap::new();
            for (k, v) in map {
                if let Some(ks) = k.as_str() {
                    m.insert(ks.to_string(), yaml_to_puppet(v));
                }
            }
            PuppetValue::Hash(m)
        }
        _ => PuppetValue::Undef,
    }
}

/// Interpolate a Hiera path template's `%{facts.a.b}` / `%{::a.b}` placeholders
/// against `facts`. Returns `None` if any placeholder can't be resolved.
fn hiera_interpolate(template: &str, facts: &PuppetValue) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("%{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        let expr = after[..end].trim();
        let dotted = expr
            .strip_prefix("facts.")
            .or_else(|| expr.strip_prefix("::"))
            .unwrap_or(expr);
        out.push_str(&hiera_navigate(facts, dotted)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn hiera_navigate(facts: &PuppetValue, dotted: &str) -> Option<String> {
    let mut current = facts;
    for part in dotted.split('.') {
        match current {
            PuppetValue::Hash(map) => current = map.get(part)?,
            _ => return None,
        }
    }
    match current {
        PuppetValue::String(s) => Some(s.clone()),
        PuppetValue::Integer(n) => Some(n.to_string()),
        _ => None,
    }
}

impl PuppetModule {
    fn load_with_fixtures(
        module_path: &Path,
        fixture_module_paths: &[std::path::PathBuf],
    ) -> Result<Self> {
        let mut classes = HashMap::new();
        let mut defines = HashMap::new();
        let mut module_paths: HashMap<String, std::path::PathBuf> = HashMap::new();
        let mut type_aliases: HashMap<String, TypeSpec> = HashMap::new();

        // Load fixtures first so the primary module's defs win on conflict.
        for fixture_path in fixture_module_paths {
            register_module_path(&mut module_paths, fixture_path);
            load_type_aliases_into(fixture_path, &mut type_aliases);
            // Be tolerant: a single broken fixture should not block the run.
            let mut sink = Vec::new();
            if let Err(err) =
                load_manifests_into(fixture_path, &mut classes, &mut defines, &mut sink)
            {
                eprintln!(
                    "warning: skipping fixture module {}: {}",
                    fixture_path.display(),
                    err
                );
            }
        }

        register_module_path(&mut module_paths, &module_path.to_path_buf());
        let mut primary_manifest_files = Vec::new();
        load_manifests_into(
            module_path,
            &mut classes,
            &mut defines,
            &mut primary_manifest_files,
        )
        .with_context(|| format!("load manifests for {}", module_path.display()))?;
        primary_manifest_files.sort();
        // Primary module's aliases overwrite any same-named fixture alias.
        load_type_aliases_into(module_path, &mut type_aliases);
        let hiera = Hiera::load(module_path);

        Ok(Self {
            classes,
            defines,
            module_paths,
            primary_manifest_files,
            type_aliases,
            hiera,
        })
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

/// Load every `type Foo::Bar = <type>` alias under `<module_path>/types/`
/// (recursively) into `aliases`. Best-effort: an unreadable or unparseable
/// file is skipped rather than failing the run.
fn load_type_aliases_into(module_path: &Path, aliases: &mut HashMap<String, TypeSpec>) {
    let mut stack = vec![module_path.join("types")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("pp") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some((name, spec)) = parse_type_alias(&content) {
                aliases.insert(name, spec);
            }
        }
    }
}

/// Parse a single `type <Name> = <type-expression>` statement out of a type
/// alias file (comments and surrounding whitespace are ignored). Returns the
/// alias name and its parsed [`TypeSpec`].
fn parse_type_alias(source: &str) -> Option<(String, TypeSpec)> {
    let mut parser = PuppetParser::new(source);
    // Skip to the `type` keyword (comments aren't tokenized, so it's normally
    // the very first token).
    loop {
        if parser.is_eof() {
            return None;
        }
        let is_type_kw = parser.peek_kind() == Some(TokenKind::Ident)
            && parser.peek_token().map(|t| t.text.as_str()) == Some("type");
        if is_type_kw {
            parser.index += 1;
            break;
        }
        parser.index += 1;
    }
    let name = parser.expect_ident().ok()?;
    if !parser.consume(TokenKind::Equal) {
        return None;
    }
    let spec = parser.parse_type_value().ok()?;
    Some((name, spec))
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
        let is_dir = std::fs::metadata(&path)
            .map(|m| m.is_dir())
            .unwrap_or(false);
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
    /// Declared data type per parameter (`Enum['a','b'] $x`), when annotated.
    /// Used to validate passed/defaulted values during compilation.
    param_types: HashMap<String, TypeSpec>,
    /// Parameters declared without a default value (required at the call site).
    required_params: HashSet<String>,
    parent: Option<String>,
    body: Vec<Stmt>,
    origin_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct DefineDef {
    name: String,
    params: HashMap<String, Expr>,
    param_types: HashMap<String, TypeSpec>,
    required_params: HashSet<String>,
    body: Vec<Stmt>,
    origin_file: Option<PathBuf>,
}

/// The parsed parameter list of a class or define: default-value expressions
/// and (where annotated) declared data types, keyed by parameter name, plus
/// the set of parameters declared without a default (required).
struct ParsedParams {
    params: HashMap<String, Expr>,
    types: HashMap<String, TypeSpec>,
    required: HashSet<String>,
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
    /// A resource default: `File { owner => 'root', mode => '0644' }`. A
    /// capitalized type reference followed by an attribute block with no
    /// title. It sets default attribute values inherited by every resource of
    /// that type declared afterwards (for any attribute the declaration itself
    /// does not set). Distinct from a normal declaration (`file { 'x': ... }`,
    /// lowercase type + title) and a resource override
    /// (`File['x'] { ... }`, capitalized type + `[title]`).
    ResourceDefault {
        rtype: String,
        attrs: HashMap<String, Expr>,
    },
    /// `include`/`contain`/`require` of one or more classes. All three declare
    /// the named class(es) into the catalog and expand their bodies; we don't
    /// model the ordering/containment nuances that distinguish them, so they
    /// share a single statement form. Holds a list so the comma-separated and
    /// array call forms (`include foo, bar`, `contain ['a', 'b']`) all work.
    Include(Vec<String>),
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
    /// `$iterable.each |$a[, $b]| { body }` — iterates an array or hash,
    /// binding the lambda parameters to each element (or key/value pair) and
    /// running the body. Modeled as a first-class statement so that resource
    /// declarations inside the body land in the catalog.
    EachLoop {
        iterable: Expr,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    /// `create_resources('rtype', $hash[, $defaults])` — iterates the hash
    /// and declares one resource per entry. Modeled as a first-class statement
    /// so the evaluator can autoload the target defined type and expand its
    /// body into the catalog (otherwise child resources of `mod::foo` would
    /// never appear under e.g. `contain_exec(...)`).
    CreateResources {
        rtype_expr: Expr,
        hash_expr: Expr,
        defaults_expr: Option<Expr>,
    },
    /// `ensure_resource($type, $title[, $params])` — stdlib's idempotent
    /// resource declaration. Declares one resource (or one per title, when
    /// `$title` is an array) of the given type with the given params, and
    /// expands the matching defined type if `$type` names one. Modeled as a
    /// first-class statement because the call almost always carries a `{...}`
    /// hash argument: without explicit parsing the hash's closing `}` is
    /// mistaken for the enclosing class body's `}`, silently truncating the
    /// rest of the class.
    EnsureResource {
        type_expr: Expr,
        title_expr: Expr,
        params_expr: Option<Expr>,
    },
    /// `ensure_packages($packages[, $params])` — declares a `package` resource
    /// per entry. `$packages` is either an array of names (each taking the
    /// shared `$params` as defaults) or a hash of `name => per-package params`
    /// (merged over the defaults). As with `ensure_resource`, the trailing
    /// `{...}` argument forces first-class parsing to avoid truncating the
    /// class body.
    EnsurePackages {
        packages_expr: Expr,
        params_expr: Option<Expr>,
    },
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
    Defined {
        rtype: String,
        title: String,
    },
    Compare {
        left: Expr,
        op: CompareOp,
        right: Expr,
    },
    /// `<needle> in <haystack>` — membership test against arrays, hash keys, or
    /// substring of a string.
    In {
        needle: Expr,
        haystack: Expr,
    },
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
    MethodCall {
        target: Box<Expr>,
        name: String,
    },
    /// A method call carrying a lambda block in *value* position, e.g.
    /// `$xs.map |$x| { "${x}!" }` used as a selector arm, attribute value, or
    /// function argument. `body` is the lambda's return expression (the common
    /// single-expression form; extra statements in the block are skipped during
    /// parsing). Statement-position `.each` is modeled separately as
    /// `Stmt::EachLoop` so its body can declare resources.
    Lambda {
        target: Box<Expr>,
        method: String,
        params: Vec<String>,
        body: Box<Expr>,
    },
    ResourceRef {
        rtype: String,
        title: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
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
    /// A Puppet data-type reference such as `String`, `String[1,10]`,
    /// `Integer[0,default]`, `Optional[Array[String]]`, or `Enum['a','b']`.
    /// Used as the right-hand side of `=~`/`!~`, as a `case`/selector branch,
    /// where it performs structural type matching rather than regex matching.
    Type(TypeSpec),
}

/// A parsed Puppet data type and its bracketed arguments. Bounds, element
/// types, and enum/pattern members are all captured as [`TypeArg`]s so the
/// evaluator can enforce them (e.g. `String[1,10]` length bounds).
#[derive(Debug, Clone)]
struct TypeSpec {
    name: String,
    args: Vec<TypeArg>,
}

#[derive(Debug, Clone)]
enum TypeArg {
    /// A numeric bound or literal (e.g. the `1`/`10` in `String[1,10]`).
    Int(i64),
    /// The `default` keyword, meaning "unbounded" in this position.
    Default,
    /// A string literal member (e.g. an `Enum['stopped','running']` value).
    Str(String),
    /// A regex member (e.g. a `Pattern[/^\d+$/]` alternative).
    Regex(String),
    /// A nested type (e.g. the `String` in `Array[String]`).
    Type(TypeSpec),
    /// The field set of a `Struct[{ ... }]` type.
    Struct(Vec<StructField>),
}

/// One key of a `Struct` type: `Optional['name'] => <type>` or
/// `'name' => <type>`. `optional` is true when the key is wrapped in
/// `Optional[...]` (the key may be absent from a matching hash).
#[derive(Debug, Clone)]
struct StructField {
    key: String,
    optional: bool,
    value: TypeSpec,
}

impl TypeSpec {
    /// Render the type back to source-like text for the rare case a type
    /// reference is used in value position rather than as a match pattern.
    fn render(&self) -> String {
        if self.args.is_empty() {
            return self.name.clone();
        }
        let args = self
            .args
            .iter()
            .map(|arg| match arg {
                TypeArg::Int(n) => n.to_string(),
                TypeArg::Default => "default".to_string(),
                TypeArg::Str(s) => format!("'{s}'"),
                TypeArg::Regex(r) => format!("/{r}/"),
                TypeArg::Type(spec) => spec.render(),
                TypeArg::Struct(fields) => {
                    let body = fields
                        .iter()
                        .map(|f| {
                            let key = if f.optional {
                                format!("Optional['{}']", f.key)
                            } else {
                                format!("'{}'", f.key)
                            };
                            format!("{} => {}", key, f.value.render())
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{{ {body} }}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}[{}]", self.name, args)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
struct VarRef {
    name: String,
    /// Index expressions applied to the variable, e.g. the `$k` and `'name'`
    /// in `$data[$k]['name']`. Evaluated at resolve time so dynamic keys —
    /// including integer keys and variable keys — resolve correctly.
    path: Vec<Expr>,
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
    /// Resource defaults (`File { owner => 'root' }`), keyed by normalized
    /// type. Merged into each subsequently-declared resource of that type for
    /// any attribute the declaration does not set itself.
    resource_defaults: HashMap<String, HashMap<String, PuppetValue>>,
    /// The class under test, if the subject is a class. Only this class's
    /// parameters are type-validated — classes pulled in via `include` (whether
    /// from the subject class or from a defined type) are trusted.
    subject_class: Option<String>,
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
            resource_defaults: HashMap::new(),
            subject_class: None,
        }
    }

    fn into_trace(self) -> EvaluationTrace {
        EvaluationTrace {
            classes: self.evaluated_classes,
            defines: self.evaluated_defines,
        }
    }

    /// Validate the parameters of the class/define under test against their
    /// declared data types. To avoid false negatives on the many valid-value
    /// compiles, only values *explicitly passed* via `let(:params)` are
    /// type-checked (declared defaults are trusted); required parameters with
    /// no supplied value raise the way Puppet does. Error text mirrors Puppet's
    /// (`expects a <Type> value` plus the `Error while evaluating a Resource
    /// Statement` wrapper) so spec `raise_error(/…/)` message constraints match.
    fn validate_subject_params(
        &self,
        subject: &str,
        param_types: &HashMap<String, TypeSpec>,
        required: &HashSet<String>,
        passed: &HashSet<String>,
    ) -> Result<()> {
        for name in required {
            if !passed.contains(name) {
                return Err(anyhow::anyhow!(
                    "{subject}: expects a value for parameter '{name}'"
                ));
            }
        }
        for (name, ty) in param_types {
            if !passed.contains(name) {
                continue;
            }
            let value = self.vars.get(name).cloned().unwrap_or(PuppetValue::Undef);
            if !eval_type_match(&value, ty, &self.module.type_aliases) {
                return Err(anyhow::anyhow!(
                    "{subject}: parameter '{name}' {}, got {} \
                     (Error while evaluating a Resource Statement)",
                    expects_clause(ty, &self.module.type_aliases),
                    value.as_string()
                ));
            }
        }
        Ok(())
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
        // Automatic class-parameter lookup from Hiera: for each parameter not
        // explicitly passed, a `<class>::<param>` data value overrides the
        // manifest default (Puppet precedence: passed > Hiera > default).
        let passed_keys: HashSet<String> = match &self.params {
            PuppetValue::Hash(h) => h.keys().cloned().collect(),
            _ => HashSet::new(),
        };
        for param in class_def.params.keys() {
            if passed_keys.contains(param) {
                continue;
            }
            let key = format!("{name}::{param}");
            if let Some(value) = self.module.hiera.lookup(&key, &self.facts) {
                // A null data value is treated as "not set" so the manifest
                // default stands (avoids clobbering e.g. `Array $x = []`).
                if !matches!(value, PuppetValue::Undef) {
                    local_vars.insert(param.clone(), value);
                }
            }
        }
        self.apply_param_overrides(&mut local_vars)?;
        self.vars.extend(local_vars);

        // Validate parameters only for the class actually under test, never for
        // classes pulled in via `include` (from the subject class or a define).
        if self.subject_class.as_deref() == Some(name) {
            let passed = match &self.params {
                PuppetValue::Hash(h) => h.keys().cloned().collect(),
                _ => HashSet::new(),
            };
            self.validate_subject_params(
                &format!("Class[{name}]"),
                &class_def.param_types,
                &class_def.required_params,
                &passed,
            )?;
        }

        // Publish each class parameter under its fully-qualified name
        // (`class::param`) so other classes can read it as `$class::param`.
        // Body-assigned variables already get this treatment in the VarAssign
        // handler; without doing the same for parameters, a cross-class
        // reference like `$ferrogate::user` would resolve to Undef even though
        // `$ferrogate::config_dir` (a body variable) works.
        for param in class_def.params.keys() {
            if let Some(value) = self.vars.get(param).cloned() {
                self.vars.insert(format!("{name}::{param}"), value);
            }
        }

        self.catalog.add(PuppetResource {
            resource_type: "class".to_string(),
            title: name.to_string(),
            attributes: IndexMap::new(),
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
        attrs: &IndexMap<String, PuppetValue>,
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
        self.vars
            .insert("title".to_string(), PuppetValue::String(title.to_string()));
        self.vars
            .insert("name".to_string(), PuppetValue::String(title.to_string()));
        let mut local_vars = HashMap::new();
        self.apply_param_defaults(&define_def.params, &mut local_vars)?;
        for (key, value) in attrs {
            local_vars.insert(key.clone(), value.clone());
        }
        self.vars.extend(local_vars);
        // Surface the define's *resolved* parameters on the catalog resource:
        // the values passed at the call site, plus any declared parameter that
        // fell back to its default. Without this, a parameter like `ports` that
        // relies on its default reads back as Undef from the catalog matcher.
        // Snapshot before evaluating the body so body-local reassignments to a
        // same-named variable don't leak into the resource's attributes.
        let mut resolved_attrs = attrs.clone();
        for param in define_def.params.keys() {
            if !resolved_attrs.contains_key(param) {
                if let Some(value) = self.vars.get(param) {
                    resolved_attrs.insert(param.clone(), value.clone());
                }
            }
        }
        self.catalog.add(PuppetResource {
            resource_type: name.to_string(),
            title: title.to_string(),
            attributes: resolved_attrs,
        });
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
        // Within a defined type both `$title` and `$name` default to the
        // resource title (a `name` parameter can override `$name`, applied via
        // param defaults/overrides below).
        self.vars
            .insert("title".to_string(), PuppetValue::String(title.to_string()));
        self.vars
            .insert("name".to_string(), PuppetValue::String(title.to_string()));
        let mut local_vars = HashMap::new();
        self.apply_param_defaults(&define_def.params, &mut local_vars)?;
        self.apply_param_overrides(&mut local_vars)?;
        self.vars.extend(local_vars);

        // Validate the define-under-test's parameters (see
        // validate_subject_params); `name` here is the literal title-less
        // define name, so reference it as the resource being declared.
        let passed = match &self.params {
            PuppetValue::Hash(h) => h.keys().cloned().collect(),
            _ => HashSet::new(),
        };
        self.validate_subject_params(
            &format!("{}[{title}]", normalize_rtype(name)),
            &define_def.param_types,
            &define_def.required_params,
            &passed,
        )?;

        // The define under test is itself a catalog resource, carrying its
        // resolved parameters (passed values merged with declared defaults), so
        // `contain_<type>('title').with_<param>(...)` can match — including
        // parameters left at their default, which would otherwise be Undef.
        let mut resolved_attrs = IndexMap::new();
        if let PuppetValue::Hash(params) = &self.params {
            for (key, value) in params {
                resolved_attrs.insert(key.clone(), value.clone());
            }
        }
        for param in define_def.params.keys() {
            if !resolved_attrs.contains_key(param) {
                if let Some(value) = self.vars.get(param) {
                    resolved_attrs.insert(param.clone(), value.clone());
                }
            }
        }
        self.catalog.add(PuppetResource {
            resource_type: normalize_rtype(name),
            title: title.to_string(),
            attributes: resolved_attrs,
        });
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
                        let scoped_value =
                            self.vars.get(name).cloned().unwrap_or(PuppetValue::Undef);
                        self.vars.insert(scoped, scoped_value);
                    }
                }
                Stmt::Resource {
                    rtype,
                    titles,
                    attrs,
                } => {
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
                    let mut attributes = IndexMap::new();
                    for (key, expr) in attrs {
                        attributes.insert(key.clone(), self.eval_expr(expr)?);
                    }
                    // Inherit any resource defaults (`File { ... }`) for
                    // attributes the declaration did not set itself.
                    if let Some(defaults) = self.resource_defaults.get(&normalize_rtype(rtype)) {
                        for (key, value) in defaults {
                            attributes
                                .entry(key.clone())
                                .or_insert_with(|| value.clone());
                        }
                    }
                    for title in title_values {
                        let resource_type = normalize_rtype(rtype);
                        if resource_type == "class" {
                            // Expand the class body first — `evaluate_class` adds
                            // a bare (attribute-less) class resource — then add
                            // the declaration's resolved attributes so it wins
                            // the catalog slot. Otherwise the parameters passed
                            // at `class { 'x': p => v }` are lost and matchers
                            // can't introspect them.
                            let _ = self.evaluate_class(&title);
                            self.catalog.add(PuppetResource {
                                resource_type,
                                title: title.clone(),
                                attributes: attributes.clone(),
                            });
                            continue;
                        }
                        self.catalog.add(PuppetResource {
                            resource_type: resource_type.clone(),
                            title: title.clone(),
                            attributes: attributes.clone(),
                        });
                        if self.module.defines.contains_key(&resource_type) {
                            let _ = self.instantiate_define(&resource_type, &title, &attributes);
                        }
                    }
                }
                Stmt::Include(names) => {
                    for name in names {
                        let _ = self.evaluate_class(name);
                    }
                }
                Stmt::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    if self.eval_cond(cond)? {
                        self.evaluate_statements(then_body)?;
                    } else {
                        self.evaluate_statements(else_body)?;
                    }
                }
                Stmt::Case {
                    expr,
                    branches,
                    default,
                } => {
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
                Stmt::EachLoop {
                    iterable,
                    params,
                    body,
                } => {
                    let value = self.eval_expr(iterable)?;
                    // Snapshot bindings the loop may overwrite so the
                    // surrounding scope is restored once iteration ends.
                    let saved: Vec<(String, Option<PuppetValue>)> = params
                        .iter()
                        .map(|name| (name.clone(), self.vars.get(name).cloned()))
                        .collect();
                    match value {
                        PuppetValue::Array(items) => {
                            for item in items {
                                if let Some(name) = params.first() {
                                    self.vars.insert(name.clone(), item);
                                }
                                self.evaluate_statements(body)?;
                            }
                        }
                        PuppetValue::Hash(entries) => {
                            for (key, val) in entries {
                                match params.as_slice() {
                                    [k] => {
                                        self.vars.insert(k.clone(), PuppetValue::String(key));
                                    }
                                    [k, v, ..] => {
                                        self.vars.insert(k.clone(), PuppetValue::String(key));
                                        self.vars.insert(v.clone(), val);
                                    }
                                    [] => {}
                                }
                                self.evaluate_statements(body)?;
                            }
                        }
                        _ => {}
                    }
                    for (name, prior) in saved {
                        match prior {
                            Some(v) => {
                                self.vars.insert(name, v);
                            }
                            None => {
                                self.vars.remove(&name);
                            }
                        }
                    }
                }
                Stmt::CreateResources {
                    rtype_expr,
                    hash_expr,
                    defaults_expr,
                } => {
                    let resource_type = normalize_rtype(&self.eval_expr(rtype_expr)?.as_string());
                    let entries = match self.eval_expr(hash_expr)? {
                        PuppetValue::Hash(entries) => entries,
                        _ => continue,
                    };
                    let defaults = match defaults_expr {
                        Some(expr) => match self.eval_expr(expr)? {
                            PuppetValue::Hash(map) => map,
                            _ => IndexMap::new(),
                        },
                        None => IndexMap::new(),
                    };
                    for (title, attrs_value) in entries {
                        let mut attributes: IndexMap<String, PuppetValue> = defaults.clone();
                        if let PuppetValue::Hash(attrs) = attrs_value {
                            for (key, value) in attrs {
                                attributes.insert(key, value);
                            }
                        }
                        self.catalog.add(PuppetResource {
                            resource_type: resource_type.clone(),
                            title: title.clone(),
                            attributes: attributes.clone(),
                        });
                        if resource_type == "class" {
                            let _ = self.evaluate_class(&title);
                        } else if self.module.defines.contains_key(&resource_type) {
                            let _ = self.instantiate_define(&resource_type, &title, &attributes);
                        }
                    }
                }
                Stmt::EnsureResource {
                    type_expr,
                    title_expr,
                    params_expr,
                } => {
                    let resource_type = normalize_rtype(&self.eval_expr(type_expr)?.as_string());
                    let titles = match self.eval_expr(title_expr)? {
                        PuppetValue::Array(items) => {
                            items.into_iter().map(|v| v.as_string()).collect()
                        }
                        other => vec![other.as_string()],
                    };
                    let attributes = match params_expr {
                        Some(expr) => match self.eval_expr(expr)? {
                            PuppetValue::Hash(map) => map,
                            _ => IndexMap::new(),
                        },
                        None => IndexMap::new(),
                    };
                    for title in titles {
                        self.catalog.add(PuppetResource {
                            resource_type: resource_type.clone(),
                            title: title.clone(),
                            attributes: attributes.clone(),
                        });
                        if self.module.defines.contains_key(&resource_type) {
                            let _ = self.instantiate_define(&resource_type, &title, &attributes);
                        }
                    }
                }
                Stmt::EnsurePackages {
                    packages_expr,
                    params_expr,
                } => {
                    let defaults = match params_expr {
                        Some(expr) => match self.eval_expr(expr)? {
                            PuppetValue::Hash(map) => map,
                            _ => IndexMap::new(),
                        },
                        None => IndexMap::new(),
                    };
                    // stdlib's `ensure_packages` defaults each package to
                    // `ensure => present` unless overridden by the params.
                    let with_ensure_default = |mut attrs: IndexMap<String, PuppetValue>| {
                        attrs
                            .entry("ensure".to_string())
                            .or_insert_with(|| PuppetValue::String("present".to_string()));
                        attrs
                    };
                    match self.eval_expr(packages_expr)? {
                        // Array of package names — each takes the shared params.
                        PuppetValue::Array(items) => {
                            for item in items {
                                self.catalog.add(PuppetResource {
                                    resource_type: "package".to_string(),
                                    title: item.as_string(),
                                    attributes: with_ensure_default(defaults.clone()),
                                });
                            }
                        }
                        // Hash of `name => per-package params` over the defaults.
                        PuppetValue::Hash(entries) => {
                            for (title, attrs_value) in entries {
                                let mut attributes = defaults.clone();
                                if let PuppetValue::Hash(attrs) = attrs_value {
                                    for (key, value) in attrs {
                                        attributes.insert(key, value);
                                    }
                                }
                                self.catalog.add(PuppetResource {
                                    resource_type: "package".to_string(),
                                    title,
                                    attributes: with_ensure_default(attributes),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Stmt::ResourceDefault { rtype, attrs } => {
                    let resource_type = normalize_rtype(rtype);
                    let mut evaluated = HashMap::new();
                    for (key, expr) in attrs {
                        evaluated.insert(key.clone(), self.eval_expr(expr)?);
                    }
                    self.resource_defaults
                        .entry(resource_type)
                        .or_default()
                        .extend(evaluated);
                }
                Stmt::Noop => {}
            }
        }
        Ok(())
    }

    /// Evaluate `left =~ right`. When the right-hand side is a data-type
    /// reference (`String[1,10]`, `Enum[...]`, …) this performs structural type
    /// matching with bounds enforcement; otherwise it falls back to compiling
    /// the right-hand value as a regex.
    fn eval_match(&mut self, left: &Expr, right: &Expr) -> Result<bool> {
        let subject = self.eval_expr(left)?;
        if let Expr::Type(spec) = right {
            return Ok(eval_type_match(&subject, spec, &self.module.type_aliases));
        }
        eval_regex_match(&subject, &self.eval_expr(right)?)
    }

    /// Match a `case` branch pattern against the subject value.
    ///
    /// Supported branch patterns:
    /// - bare regex literal `/.../[imsx]` — substring match on the subject
    /// - data type reference `String[1,10]`, `Enum[...]` — structural type match
    /// - array of patterns `[a, b, /c/, ...]` — true if any element matches
    /// - any other expression — equality after evaluation
    fn case_branch_matches(&mut self, pattern: &Expr, subject: &PuppetValue) -> Result<bool> {
        match pattern {
            Expr::Regex(regex_src) => {
                eval_regex_match(subject, &PuppetValue::String(regex_src.clone()))
            }
            Expr::Type(spec) => Ok(eval_type_match(subject, spec, &self.module.type_aliases)),
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
            Cond::Compare { left, op, right } => match op {
                CompareOp::Eq => self.eval_expr(left)? == self.eval_expr(right)?,
                CompareOp::NotEq => self.eval_expr(left)? != self.eval_expr(right)?,
                CompareOp::Match => self.eval_match(left, right)?,
                CompareOp::NotMatch => !self.eval_match(left, right)?,
                CompareOp::Lt | CompareOp::Gt | CompareOp::LtEq | CompareOp::GtEq => {
                    eval_ordered_compare(&self.eval_expr(left)?, *op, &self.eval_expr(right)?)
                }
            },
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
                let mut map = IndexMap::new();
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
                    "length" | "size" => PuppetValue::Integer(value_length(&value)),
                    "empty" => PuppetValue::Bool(value_empty(&value)),
                    _ => value,
                }
            }
            Expr::Lambda {
                target,
                method,
                params,
                body,
            } => self.eval_lambda(target, method, params, body)?,
            Expr::ResourceRef { rtype, title } => {
                // `Package[$packages]` where `$packages` is an array references
                // one resource per element (`Package[a], Package[b]`), so expand
                // an array title into an array of references.
                match self.eval_expr(title)? {
                    PuppetValue::Array(items) => PuppetValue::Array(
                        items
                            .iter()
                            .map(|t| PuppetValue::String(format!("{rtype}[{}]", t.as_string())))
                            .collect(),
                    ),
                    other => PuppetValue::String(format!("{rtype}[{}]", other.as_string())),
                }
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
                            .unwrap_or_else(|| PuppetValue::Hash(IndexMap::new()));
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
                            .unwrap_or_else(|| PuppetValue::Hash(IndexMap::new()));
                        PuppetValue::String(render_epp(&template, &params))
                    }
                    "template" => {
                        // `template('mod/file.erb')` — render the named ERB file
                        // against the current scope. Falls back to a placeholder
                        // when the file can't be found/read.
                        let reference = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        match self
                            .resolve_template_file(&reference)
                            .and_then(|path| std::fs::read_to_string(path).ok())
                        {
                            Some(tpl) => PuppetValue::String(render_erb(&tpl, &self.vars)),
                            None => PuppetValue::String(format!("<template:{reference}>")),
                        }
                    }
                    "inline_template" => {
                        let tpl = arg_values
                            .first()
                            .map(|value| value.as_string())
                            .unwrap_or_default();
                        PuppetValue::String(render_erb(&tpl, &self.vars))
                    }
                    // stdlib `dirname`/`basename` — used e.g. to derive an
                    // include directory from a glob path (`dirname($include)`).
                    "dirname" => match arg_values.first() {
                        Some(PuppetValue::String(p)) => {
                            let trimmed = p.trim_end_matches('/');
                            let dir = trimmed.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                            PuppetValue::String(dir.to_string())
                        }
                        _ => PuppetValue::Undef,
                    },
                    "basename" => match arg_values.first() {
                        Some(PuppetValue::String(p)) => {
                            let trimmed = p.trim_end_matches('/');
                            let base = trimmed.rsplit_once('/').map(|(_, b)| b).unwrap_or(trimmed);
                            PuppetValue::String(base.to_string())
                        }
                        _ => PuppetValue::Undef,
                    },
                    // stdlib `concat(a, b, …)` — concatenate arrays/values into a
                    // single array (non-array args are appended as elements).
                    "concat" => {
                        let mut out = Vec::new();
                        for value in &arg_values {
                            match value {
                                PuppetValue::Array(items) => out.extend(items.iter().cloned()),
                                other => out.push(other.clone()),
                            }
                        }
                        PuppetValue::Array(out)
                    }
                    // `join(array[, sep])` — join array elements with an optional
                    // separator (default empty string).
                    "join" => {
                        let sep = arg_values
                            .get(1)
                            .map(|v| v.as_string())
                            .unwrap_or_default();
                        match arg_values.first() {
                            Some(PuppetValue::Array(items)) => PuppetValue::String(
                                items
                                    .iter()
                                    .map(|v| v.as_string())
                                    .collect::<Vec<_>>()
                                    .join(&sep),
                            ),
                            Some(other) => PuppetValue::String(other.as_string()),
                            None => PuppetValue::Undef,
                        }
                    }
                    // `flatten(array)` — recursively flatten nested arrays.
                    "flatten" => {
                        fn flatten_into(value: &PuppetValue, out: &mut Vec<PuppetValue>) {
                            match value {
                                PuppetValue::Array(items) => {
                                    for item in items {
                                        flatten_into(item, out);
                                    }
                                }
                                other => out.push(other.clone()),
                            }
                        }
                        let mut out = Vec::new();
                        for value in &arg_values {
                            flatten_into(value, &mut out);
                        }
                        PuppetValue::Array(out)
                    }
                    // Puppet's numeric type-conversion functions. `Integer('7')`
                    // must yield the integer `7`, not `Undef` — otherwise a
                    // subsequent comparison (`Integer('7') >= 10`) falls back to
                    // lexical string comparison and mis-evaluates.
                    "Integer" | "Numeric" => arg_values
                        .first()
                        .and_then(coerce_to_integer)
                        .map(PuppetValue::Integer)
                        .unwrap_or(PuppetValue::Undef),
                    // Collection introspection. `empty`/`length`/`size` return
                    // proper Bool/Integer so comparisons (`length($x) > 0`,
                    // `empty($x)`) evaluate correctly instead of on `Undef`.
                    "empty" => {
                        PuppetValue::Bool(arg_values.first().map(value_empty).unwrap_or(true))
                    }
                    "length" | "size" => {
                        PuppetValue::Integer(arg_values.first().map(value_length).unwrap_or(0))
                    }
                    _ => PuppetValue::Undef,
                }
            }
            // Regex literals evaluate to their pattern string; comparison ops
            // `=~` / `!~` compile and apply the pattern.
            Expr::Regex(pattern) => PuppetValue::String(pattern.clone()),
            // A type reference in value position (rare) renders to its source
            // text; in match position `=~`/`!~`/`case` intercept it for
            // structural type matching before it is evaluated as a value.
            Expr::Type(spec) => PuppetValue::String(spec.render()),
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
                    if let Expr::Type(spec) = key {
                        if eval_type_match(&subject_value, spec, &self.module.type_aliases) {
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

    /// Evaluate a value-position lambda (`receiver.method |params| { body }`).
    /// Supports `map`, `filter`/`select`, `reject`, and `each`; unrecognized
    /// methods fall back to the receiver value so the surrounding expression
    /// still evaluates. The lambda parameters are bound for the body's duration
    /// and any prior bindings of the same names are restored afterwards.
    fn eval_lambda(
        &mut self,
        target: &Expr,
        method: &str,
        params: &[String],
        body: &Expr,
    ) -> Result<PuppetValue> {
        let receiver = self.eval_expr(target)?;

        // Iterate the receiver as a list of "elements", where each element is
        // the binding(s) to apply for one lambda invocation.
        let elements: Vec<Vec<PuppetValue>> = match &receiver {
            PuppetValue::Array(items) => items.iter().map(|item| vec![item.clone()]).collect(),
            PuppetValue::Hash(entries) => entries
                .iter()
                .map(|(k, v)| vec![PuppetValue::String(k.clone()), v.clone()])
                .collect(),
            _ => return Ok(receiver),
        };

        let saved: Vec<(String, Option<PuppetValue>)> = params
            .iter()
            .map(|name| (name.clone(), self.vars.get(name).cloned()))
            .collect();

        let mut mapped = Vec::new();
        let mut kept = Vec::new();
        for element in &elements {
            // A single-param lambda over a hash receives `[key, value]` as the
            // one parameter; otherwise bind positionally.
            if params.len() == 1 && element.len() == 2 {
                self.vars
                    .insert(params[0].clone(), PuppetValue::Array(element.clone()));
            } else {
                for (name, value) in params.iter().zip(element.iter()) {
                    self.vars.insert(name.clone(), value.clone());
                }
            }
            let result = self.eval_expr(body)?;
            match method {
                "map" => mapped.push(result),
                "filter" | "select" => {
                    if result.is_truthy() {
                        kept.push(element[0].clone());
                    }
                }
                "reject" => {
                    if !result.is_truthy() {
                        kept.push(element[0].clone());
                    }
                }
                _ => {}
            }
        }

        for (name, prior) in saved {
            match prior {
                Some(value) => {
                    self.vars.insert(name, value);
                }
                None => {
                    self.vars.remove(&name);
                }
            }
        }

        Ok(match method {
            "map" => PuppetValue::Array(mapped),
            "filter" | "select" | "reject" => PuppetValue::Array(kept),
            _ => receiver,
        })
    }

    /// Resolve a Puppet template reference like `"rustion/rustion.toml.epp"`
    /// to an absolute filesystem path of the form `<module>/templates/<rest>`.
    fn resolve_template_file(&self, reference: &str) -> Option<std::path::PathBuf> {
        let (module_name, rest) = reference.split_once('/')?;
        let module_root = self.module.module_paths.get(module_name)?;
        Some(module_root.join("templates").join(rest))
    }

    fn resolve_var(&mut self, var: &VarRef) -> PuppetValue {
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
            facts
                .get(&normalized)
                .cloned()
                .unwrap_or(PuppetValue::Undef)
        } else {
            PuppetValue::Undef
        };
        // Apply each `[key]` suffix, evaluating the key at runtime so dynamic,
        // integer, and variable keys resolve. Hashes are indexed by the key's
        // string form (matching how hash literals store keys); arrays are
        // indexed by integer, with Puppet-style negative offsets from the end.
        for segment in &var.path {
            let key = self.eval_expr(segment).unwrap_or(PuppetValue::Undef);
            value = match value {
                PuppetValue::Hash(map) => map
                    .get(&key.as_string())
                    .cloned()
                    .unwrap_or(PuppetValue::Undef),
                PuppetValue::Array(items) => match coerce_to_integer(&key) {
                    Some(idx) => {
                        let resolved = if idx < 0 {
                            items.len() as i64 + idx
                        } else {
                            idx
                        };
                        if resolved >= 0 && (resolved as usize) < items.len() {
                            items[resolved as usize].clone()
                        } else {
                            PuppetValue::Undef
                        }
                    }
                    None => PuppetValue::Undef,
                },
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

pub(crate) fn normalize_rtype(rtype: &str) -> String {
    rtype.to_lowercase().replace("__", "::")
}

/// Whether a bareword is a Puppet *type reference* (e.g. `File`,
/// `Apache::Vhost`) rather than a resource-declaration type name (`file`,
/// `apache::vhost`). Type references capitalize the first letter; the
/// distinction is what separates a resource default from a declaration.
fn is_type_reference(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Whether a capitalized name is one of Puppet's built-in *data types* (as
/// opposed to a resource type like `File` or a class name). Only these are
/// parsed as type references; everything else capitalized stays a resource
/// reference so `File['/etc/x']` is unaffected.
fn is_data_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Integer"
            | "Float"
            | "Numeric"
            | "Boolean"
            | "Array"
            | "Hash"
            | "Optional"
            | "NotUndef"
            | "Undef"
            | "Enum"
            | "Pattern"
            | "Regexp"
            | "Variant"
            | "Scalar"
            | "ScalarData"
            | "Data"
            | "Collection"
            | "Any"
            | "Struct"
            | "Tuple"
            | "Default"
            | "Sensitive"
            | "Type"
    )
}

/// Length of a value the way Puppet's `length`/`size` report it: characters for
/// strings, element count for arrays, entry count for hashes; `0` otherwise.
fn value_length(value: &PuppetValue) -> i64 {
    match value {
        PuppetValue::String(s) => s.chars().count() as i64,
        PuppetValue::Array(items) => items.len() as i64,
        PuppetValue::Hash(map) => map.len() as i64,
        _ => 0,
    }
}

/// Whether a value is empty the way Puppet's `empty` reports it. `undef` is
/// treated as empty (matching modern stdlib), and non-collection scalars are
/// non-empty.
fn value_empty(value: &PuppetValue) -> bool {
    match value {
        PuppetValue::String(s) => s.is_empty(),
        PuppetValue::Array(items) => items.is_empty(),
        PuppetValue::Hash(map) => map.is_empty(),
        PuppetValue::Undef => true,
        _ => false,
    }
}

/// Read a `(min, max)` pair of optional bounds from type arguments starting at
/// `offset`. `Int` args set a bound; `Default` or a missing arg means
/// unbounded.
fn type_bounds(args: &[TypeArg], offset: usize) -> (Option<i64>, Option<i64>) {
    let bound = |arg: Option<&TypeArg>| match arg {
        Some(TypeArg::Int(n)) => Some(*n),
        _ => None,
    };
    (bound(args.get(offset)), bound(args.get(offset + 1)))
}

/// Whether a type is pattern-based (Pattern/Regexp/Enum, or a Variant that
/// contains one) — the kinds Puppet describes with "expects a match for …"
/// rather than "expects a <Type> value".
fn is_pattern_like(ty: &TypeSpec, aliases: &HashMap<String, TypeSpec>) -> bool {
    let r = resolve_alias(ty, aliases);
    match r.name.as_str() {
        // Puppet reports these with "expects a match for …" (Struct drills into
        // the failing key, which is itself a match-for type in practice).
        "Pattern" | "Regexp" | "Enum" | "Struct" => true,
        "Variant" => r.args.iter().any(|arg| match arg {
            TypeArg::Type(inner) => is_pattern_like(inner, aliases),
            _ => false,
        }),
        _ => false,
    }
}

/// Render a type the way Puppet prints it in a mismatch error, expanding an
/// alias to `Name = <definition>` (recursively) so e.g. `Stdlib::Absolutepath`
/// becomes `Stdlib::Absolutepath = Variant[Stdlib::Windowspath = Pattern[…],
/// Stdlib::Unixpath = Pattern[…]]`.
fn error_render(ty: &TypeSpec, aliases: &HashMap<String, TypeSpec>, depth: usize) -> String {
    if depth > 12 {
        return ty.name.clone();
    }
    // Alias reference: `Name = <expansion>`.
    if !is_data_type(&ty.name) {
        if let Some(def) = aliases.get(&ty.name) {
            return format!("{} = {}", ty.name, error_render(def, aliases, depth + 1));
        }
        return ty.name.clone();
    }
    if ty.args.is_empty() {
        return ty.name.clone();
    }
    let args = ty
        .args
        .iter()
        .map(|arg| match arg {
            TypeArg::Type(inner) => error_render(inner, aliases, depth + 1),
            TypeArg::Str(s) => format!("'{s}'"),
            TypeArg::Int(n) => n.to_string(),
            TypeArg::Regex(r) => format!("/{r}/"),
            TypeArg::Default => "default".to_string(),
            TypeArg::Struct(_) => "...".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}[{}]", ty.name, args)
}

/// Render the `expects …` fragment of a parameter type-mismatch error so it
/// matches the phrasings Puppet emits (which module specs assert against):
/// "expects a match for <expanded type>" for pattern-based types, "expects a
/// value of type Undef or <T>" for `Optional[T]` of a plain type, otherwise
/// "expects a <Type> value". For `Optional[X]` the validation only fires on a
/// non-undef value, so a pattern-based inner `X` is described directly (the way
/// Puppet reports the failing branch).
fn expects_clause(ty: &TypeSpec, aliases: &HashMap<String, TypeSpec>) -> String {
    let resolved = resolve_alias(ty, aliases);
    if resolved.name == "Optional" {
        let inner = resolved.args.iter().find_map(|arg| match arg {
            TypeArg::Type(spec) => Some(spec.clone()),
            _ => None,
        });
        return match inner {
            Some(inner) if is_pattern_like(&inner, aliases) => {
                format!("expects a match for {}", error_render(&inner, aliases, 0))
            }
            Some(inner) => format!(
                "expects a value of type Undef or {}",
                resolve_alias(&inner, aliases).name
            ),
            None => "expects a value of type Undef".to_string(),
        };
    }
    if is_pattern_like(ty, aliases) {
        return format!("expects a match for {}", error_render(ty, aliases, 0));
    }
    // Bare type name (no size/element bounds) so `String[1]` reports as
    // "expects a String value", matching Puppet's basic mismatch phrasing.
    format!("expects a {} value", resolved.name)
}

/// Follow alias references (`Ssh::Yes_no` → `Enum['yes','no']`, possibly through
/// several hops) until reaching a built-in data type or an unknown name. Puppet
/// forbids recursive aliases, so the chain is finite; a step cap guards against
/// a malformed file looping.
fn resolve_alias(spec: &TypeSpec, aliases: &HashMap<String, TypeSpec>) -> TypeSpec {
    let mut current = spec.clone();
    let mut steps = 0;
    while !is_data_type(&current.name) {
        match aliases.get(&current.name) {
            Some(def) if steps < 64 => {
                current = def.clone();
                steps += 1;
            }
            _ => break,
        }
    }
    current
}

/// Whether a (resolved) type is, or contains, a plain `String`. Drives the
/// real-Puppet quirk that an optional `Struct` key whose value type admits a
/// String also accepts an explicit `undef` (e.g. `String[1]` and
/// `Variant[String[1], Integer[0]]` do; `Enum`/`Integer`/`Pattern` do not).
fn type_admits_string(spec: &TypeSpec, aliases: &HashMap<String, TypeSpec>) -> bool {
    let resolved = resolve_alias(spec, aliases);
    match resolved.name.as_str() {
        "String" => true,
        "Variant" | "Optional" => resolved.args.iter().any(|arg| {
            matches!(arg, TypeArg::Type(inner) if type_admits_string(inner, aliases))
        }),
        _ => false,
    }
}

/// Translate the Ruby regex dialect Puppet patterns are written in into the
/// subset Rust's `regex` crate accepts: `\/` (escaped delimiter) collapses to
/// `/`, `\Z` (Ruby end-of-string-before-newline) maps to `\z`, and `\0` (null)
/// to `\x00`. Patterns that still don't compile are handled by the caller.
fn ruby_regex_to_rust(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some('/') => {
                    out.push('/');
                    chars.next();
                }
                Some('Z') => {
                    out.push_str("\\z");
                    chars.next();
                }
                Some('0') => {
                    out.push_str("\\x00");
                    chars.next();
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Compile a Puppet-authored regex pattern for matching.
fn compile_puppet_regex(src: &str) -> Option<Regex> {
    Regex::new(&ruby_regex_to_rust(src)).ok()
}

/// Whether `value` matches a single type argument (used for `Variant`,
/// `Optional`, and `Array`/`Hash` element types).
fn type_arg_matches(value: &PuppetValue, arg: &TypeArg, aliases: &HashMap<String, TypeSpec>) -> bool {
    match arg {
        TypeArg::Type(spec) => eval_type_match(value, spec, aliases),
        TypeArg::Str(s) => matches!(value, PuppetValue::String(v) if v == s),
        TypeArg::Int(n) => matches!(value, PuppetValue::Integer(v) if v == n),
        TypeArg::Regex(re) => matches!(value, PuppetValue::String(text)
            if compile_puppet_regex(re).map(|r| r.is_match(text)).unwrap_or(false)),
        TypeArg::Default => true,
        // A bare struct field set in argument position has no standalone
        // meaning; treat it as unconstrained.
        TypeArg::Struct(_) => true,
    }
}

/// Structurally match a value against a Puppet data type, enforcing length and
/// numeric bounds, element types, enum/pattern membership, and `Struct` field
/// shapes. Alias references (`Ssh::Yes_no`, `Stdlib::Port`) are resolved via
/// `aliases`. A name that is neither a built-in type nor a known alias matches
/// leniently (treated as `Any`) so a test never fails purely because the
/// embedded evaluator doesn't implement an exotic type.
fn eval_type_match(value: &PuppetValue, spec: &TypeSpec, aliases: &HashMap<String, TypeSpec>) -> bool {
    let spec = resolve_alias(spec, aliases);
    let spec = &spec;
    match spec.name.as_str() {
        "Any" | "Data" | "Default" | "Type" => true,
        "Undef" => matches!(value, PuppetValue::Undef),
        "NotUndef" => !matches!(value, PuppetValue::Undef),
        "Optional" => {
            matches!(value, PuppetValue::Undef)
                || spec
                    .args
                    .first()
                    .is_none_or(|arg| type_arg_matches(value, arg, aliases))
        }
        "Sensitive" => spec
            .args
            .first()
            .is_none_or(|arg| type_arg_matches(value, arg, aliases)),
        "Boolean" => matches!(value, PuppetValue::Bool(_)),
        "String" => match value {
            PuppetValue::String(s) => {
                let len = s.chars().count() as i64;
                let (min, max) = type_bounds(&spec.args, 0);
                min.is_none_or(|m| len >= m) && max.is_none_or(|m| len <= m)
            }
            _ => false,
        },
        "Integer" => match value {
            PuppetValue::Integer(n) => {
                let (min, max) = type_bounds(&spec.args, 0);
                min.is_none_or(|m| *n >= m) && max.is_none_or(|m| *n <= m)
            }
            _ => false,
        },
        "Float" => matches!(value, PuppetValue::Float(_)),
        "Numeric" => match value {
            PuppetValue::Integer(n) => {
                let (min, max) = type_bounds(&spec.args, 0);
                min.is_none_or(|m| *n >= m) && max.is_none_or(|m| *n <= m)
            }
            PuppetValue::Float(_) => true,
            _ => false,
        },
        "Scalar" | "ScalarData" => matches!(
            value,
            PuppetValue::String(_)
                | PuppetValue::Integer(_)
                | PuppetValue::Float(_)
                | PuppetValue::Bool(_)
        ),
        "Collection" => matches!(value, PuppetValue::Array(_) | PuppetValue::Hash(_)),
        "Array" | "Tuple" => match value {
            PuppetValue::Array(items) => {
                // An optional leading type arg constrains the element type;
                // trailing integer args constrain the size.
                let elem_type = spec.args.iter().find_map(|arg| match arg {
                    TypeArg::Type(t) => Some(t),
                    _ => None,
                });
                let elems_ok = elem_type
                    .is_none_or(|t| items.iter().all(|item| eval_type_match(item, t, aliases)));
                let int_args: Vec<i64> = spec
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        TypeArg::Int(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
                let size = items.len() as i64;
                let size_ok = match int_args.as_slice() {
                    [] => true,
                    [min] => size >= *min,
                    [min, max, ..] => size >= *min && size <= *max,
                };
                elems_ok && size_ok
            }
            _ => false,
        },
        "Hash" => match value {
            PuppetValue::Hash(map) => {
                // Size bounds, when present, are the 3rd/4th args (after the
                // key/value types): `Hash[K, V, min, max]`.
                let int_args: Vec<i64> = spec
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        TypeArg::Int(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
                let size = map.len() as i64;
                match int_args.as_slice() {
                    [] => true,
                    [min] => size >= *min,
                    [min, max, ..] => size >= *min && size <= *max,
                }
            }
            _ => false,
        },
        "Enum" => match value {
            PuppetValue::String(s) => spec
                .args
                .iter()
                .any(|arg| matches!(arg, TypeArg::Str(member) if member == s)),
            _ => false,
        },
        "Pattern" | "Regexp" => match value {
            // A Puppet `Pattern` only matches String values — an Integer or
            // Boolean is never coerced to its text form for the test.
            PuppetValue::String(text) => spec.args.iter().any(|arg| match arg {
                TypeArg::Regex(re) | TypeArg::Str(re) => {
                    compile_puppet_regex(re).map(|r| r.is_match(text)).unwrap_or(false)
                }
                _ => false,
            }),
            _ => false,
        },
        "Variant" => spec
            .args
            .iter()
            .any(|arg| type_arg_matches(value, arg, aliases)),
        "Struct" => match value {
            PuppetValue::Hash(map) => {
                let Some(TypeArg::Struct(fields)) = spec.args.first() else {
                    // Struct with no captured field set — match leniently.
                    return true;
                };
                // Every present key must be a declared field and match its
                // value type. For an `Optional['k']` key, an explicit `undef`
                // value is also accepted (Puppet widens the value type to
                // include undef when the key is optional).
                for (key, val) in map {
                    match fields.iter().find(|f| &f.key == key) {
                        // `Optional['k']` makes the key's *presence* optional;
                        // when present, the value is matched strictly against
                        // its declared type. The one exception is a quirk of
                        // real Puppet that modules rely on: an optional key
                        // whose value type is a plain `String` also accepts an
                        // explicit `undef` (Enum/Integer/Pattern/alias fields do
                        // not — they reject undef strictly).
                        Some(field) => {
                            if matches!(val, PuppetValue::Undef)
                                && field.optional
                                && type_admits_string(&field.value, aliases)
                            {
                                continue;
                            }
                            if !eval_type_match(val, &field.value, aliases) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                // Every required (non-Optional) field must be present.
                fields
                    .iter()
                    .all(|f| f.optional || map.contains_key(&f.key))
            }
            _ => false,
        },
        // Unmodeled type: match leniently rather than failing.
        _ => true,
    }
}

struct PuppetParser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    index: usize,
    allow_bare_vars: bool,
    /// When true, a `.method |params| { … }` in expression position attaches a
    /// lambda (`Expr::Lambda`). Disabled while detecting a statement-position
    /// `.each` so its block is parsed as resource-declaring statements instead.
    lambda_in_expr: bool,
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
            lambda_in_expr: true,
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
            message: "deprecated leading `::` namespace prefix; modern Puppet treats it as a no-op"
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
        let ParsedParams {
            params,
            types,
            required,
        } = self.parse_param_list()?;
        let parent = if self.consume_keyword("inherits") {
            self.consume_leading_namespace_prefix();
            Some(self.expect_ident()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(ClassDef {
            name,
            params,
            param_types: types,
            required_params: required,
            parent,
            body,
            origin_file: None,
        })
    }

    fn parse_define_def(&mut self) -> Result<DefineDef> {
        let name = self.expect_ident()?;
        let ParsedParams {
            params,
            types,
            required,
        } = self.parse_param_list()?;
        let body = self.parse_block()?;
        Ok(DefineDef {
            name,
            params,
            param_types: types,
            required_params: required,
            body,
            origin_file: None,
        })
    }

    fn parse_param_list(&mut self) -> Result<ParsedParams> {
        let mut params = HashMap::new();
        let mut types = HashMap::new();
        let mut required = HashSet::new();
        if !self.consume(TokenKind::LParen) {
            return Ok(ParsedParams {
                params,
                types,
                required,
            });
        }
        while !self.consume(TokenKind::RParen) && !self.is_eof() {
            if self.peek_kind() == Some(TokenKind::Comma) {
                self.index += 1;
                continue;
            }
            // An optional data-type annotation precedes the `$var`. A leading
            // capitalized identifier starts a type (`Optional[Enum['a','b']]`,
            // `Stdlib::Port`, `Boolean`, …); capture it so the value can later
            // be validated against it.
            let mut param_type = None;
            if self.peek_kind() == Some(TokenKind::Ident) {
                if let Ok(spec) = self.parse_type_value() {
                    param_type = Some(spec);
                }
            }
            // Skip any leftover tokens (e.g. a `*` splat marker) up to the var.
            while self.peek_kind() != Some(TokenKind::Var)
                && self.peek_kind() != Some(TokenKind::Comma)
                && self.peek_kind() != Some(TokenKind::RParen)
                && !self.is_eof()
            {
                self.index += 1;
            }
            if self.peek_kind() == Some(TokenKind::Var) {
                let name = self.expect_var()?;
                if let Some(spec) = param_type {
                    types.insert(name.clone(), spec);
                }
                if self.consume(TokenKind::Equal) {
                    let expr = self.parse_expr()?;
                    params.insert(name, expr);
                } else {
                    required.insert(name.clone());
                    params.insert(name, Expr::Undef);
                }
            } else {
                self.index += 1;
            }
            self.consume(TokenKind::Comma);
        }
        Ok(ParsedParams {
            params,
            types,
            required,
        })
    }

    /// Parse `|$a[, $b][, …]|` — the parameter list of an `.each` lambda.
    /// Type annotations between `|` and a `$var` are skipped (Puppet allows
    /// `|String $x|`) so the body can bind by name without modeling types.
    fn parse_lambda_params(&mut self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        self.expect(TokenKind::Pipe)?;
        while !self.consume(TokenKind::Pipe) && !self.is_eof() {
            if self.peek_kind() == Some(TokenKind::Comma) {
                self.index += 1;
                continue;
            }
            while self.peek_kind() != Some(TokenKind::Var)
                && self.peek_kind() != Some(TokenKind::Comma)
                && self.peek_kind() != Some(TokenKind::Pipe)
                && !self.is_eof()
            {
                self.index += 1;
            }
            if self.peek_kind() == Some(TokenKind::Var) {
                params.push(self.expect_var()?);
            }
        }
        Ok(params)
    }

    /// Parse an expression with value-position lambda attachment suppressed, so
    /// a trailing `.each |..| { .. }` is left for the statement-level iterator
    /// detector (which parses the block as resource-declaring statements).
    fn parse_no_lambda_expr(&mut self) -> Result<Expr> {
        let prev = self.lambda_in_expr;
        self.lambda_in_expr = false;
        let result = self.parse_expr();
        self.lambda_in_expr = prev;
        result
    }

    /// Parse the `{ … }` body of a value-position lambda, returning its result
    /// expression. Reads the first expression inside the braces (the lambda's
    /// return value for the common single-expression body) and then skips any
    /// remaining tokens up to the matching `}`, so multi-statement bodies parse
    /// without error even though only the leading expression is modeled.
    fn parse_lambda_value_body(&mut self) -> Result<Expr> {
        self.expect(TokenKind::LBrace)?;
        let body = if self.peek_kind() == Some(TokenKind::RBrace) {
            Expr::Undef
        } else {
            self.parse_expr()?
        };
        // Skip to the brace that closes this lambda body, tracking nesting.
        let mut depth = 1;
        while depth > 0 && !self.is_eof() {
            match self.peek_kind() {
                Some(TokenKind::LBrace) => depth += 1,
                Some(TokenKind::RBrace) => depth -= 1,
                _ => {}
            }
            self.index += 1;
        }
        Ok(body)
    }

    /// If `expr` is a `.each` method call immediately followed by a lambda
    /// (`… .each |params| { body }`), consume the parameters and body and return
    /// the iteration statement. Returns `None` (consuming nothing further) when
    /// `expr` is anything else, so callers can fall back to other handling.
    fn finish_each_loop(&mut self, expr: &Expr) -> Result<Option<Stmt>> {
        if let Expr::MethodCall { target, name } = expr {
            if name == "each" && self.peek_kind() == Some(TokenKind::Pipe) {
                let params = self.parse_lambda_params()?;
                let body = self.parse_block()?;
                return Ok(Some(Stmt::EachLoop {
                    iterable: (**target).clone(),
                    params,
                    body,
                }));
            }
        }
        Ok(None)
    }

    /// Parse the `else`/`elsif` tail of an `if`/`unless` chain. `elsif C { … }`
    /// is desugared into `else { if C { … } [else { … }] }`.
    fn parse_else_chain(&mut self) -> Result<Vec<Stmt>> {
        if self.consume_keyword("elsif") {
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = self.parse_else_chain()?;
            return Ok(vec![Stmt::If {
                cond,
                then_body,
                else_body,
            }]);
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
        // `include` / `contain` / `require` all declare one or more classes.
        // We treat them identically (see `Stmt::Include`). Each accepts a
        // comma-separated list of class references, where a reference may be a
        // bare name (`foo::bar`), a quoted string, or an array of either, with
        // an optional legacy `::` prefix and optional surrounding parentheses.
        if self.consume_keyword("include")
            || self.consume_keyword("contain")
            || self.consume_keyword("require")
        {
            let names = self.parse_class_refs()?;
            return Ok(Some(Stmt::Include(names)));
        }
        if self.consume_keyword("if") {
            let cond = self.parse_cond()?;
            let then_body = self.parse_block()?;
            let else_body = self.parse_else_chain()?;
            return Ok(Some(Stmt::If {
                cond,
                then_body,
                else_body,
            }));
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
            return Ok(Some(Stmt::Case {
                expr,
                branches,
                default,
            }));
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
            let save = self.index;
            let name = self.expect_var()?;
            if self.consume(TokenKind::Equal) {
                let expr = self.parse_value_expr()?;
                return Ok(Some(Stmt::VarAssign(name, expr)));
            }
            // `$var[.method...].each |params| { body }` — Puppet's iteration form.
            // Roll back to the variable so parse_expr can consume the full
            // method chain, then detect a trailing lambda. Lambda attachment is
            // disabled here so the `.each` block is parsed as resource-declaring
            // statements rather than collapsed into an `Expr::Lambda` value.
            self.index = save;
            let expr = self.parse_no_lambda_expr()?;
            if let Some(stmt) = self.finish_each_loop(&expr)? {
                return Ok(Some(stmt));
            }
            // Bare `$var` (or unhandled trailing expr) is dropped — preserve
            // prior behavior of skipping unrecognised statement forms.
            return Ok(Some(Stmt::Noop));
        }
        // Iteration on a literal collection or parenthesized expression rather
        // than a `$variable`: `[...].each |p| { … }` and `({...}).each |p| { … }`.
        // Puppet can't use a *bare* `{...}` hash literal at statement start (it's
        // ambiguous with a resource expression), so the supported forms are an
        // array literal `[...]` or a parenthesized expression `(...)` — the
        // latter being Puppet's documented workaround for iterating a hash
        // literal, e.g. `({'a' => 1}).each |$k, $v| { … }`. Without these
        // branches the statement parser dropped the construct and its body.
        if matches!(
            self.peek_kind(),
            Some(TokenKind::LBracket | TokenKind::LParen)
        ) {
            let expr = self.parse_no_lambda_expr()?;
            if let Some(stmt) = self.finish_each_loop(&expr)? {
                return Ok(Some(stmt));
            }
            // A bare collection/parenthesized expression as a statement has no
            // effect; consume it.
            return Ok(Some(Stmt::Noop));
        }
        // `create_resources('rtype', $hash[, $defaults])` as a statement.
        // Recognize the bare identifier followed by `(` so the call's side
        // effect (declaring resources) actually runs — otherwise the parser
        // skips the tokens and the defined type body never expands.
        if self.peek_kind() == Some(TokenKind::Ident)
            && self
                .tokens
                .get(self.index)
                .map(|t| t.text == "create_resources")
                .unwrap_or(false)
            && self
                .tokens
                .get(self.index + 1)
                .map(|t| t.kind == TokenKind::LParen)
                .unwrap_or(false)
        {
            self.index += 2; // consume `create_resources` and `(`
            let rtype_expr = self.parse_expr()?;
            self.consume(TokenKind::Comma);
            let hash_expr = self.parse_expr()?;
            let defaults_expr = if self.consume(TokenKind::Comma) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.consume(TokenKind::RParen);
            return Ok(Some(Stmt::CreateResources {
                rtype_expr,
                hash_expr,
                defaults_expr,
            }));
        }
        // `ensure_resource(...)` / `ensure_packages(...)` as statements. Like
        // `create_resources`, the call's side effect (declaring resources) must
        // run, and — crucially — its arguments must be parsed as expressions so
        // a trailing `{...}` hash is consumed whole. Otherwise the parser leaves
        // the `(` behind, then mistakes the hash argument's closing `}` for the
        // class body's `}` and drops every statement that follows the call.
        if self.peek_kind() == Some(TokenKind::Ident)
            && self
                .tokens
                .get(self.index + 1)
                .map(|t| t.kind == TokenKind::LParen)
                .unwrap_or(false)
        {
            let fname = self.tokens[self.index].text.clone();
            if fname == "ensure_resource" || fname == "ensure_packages" {
                self.index += 2; // consume the function name and `(`
                let mut args = Vec::new();
                while !self.consume(TokenKind::RParen) && !self.is_eof() {
                    if self.peek_kind() == Some(TokenKind::Comma) {
                        self.index += 1;
                        continue;
                    }
                    args.push(self.parse_expr()?);
                }
                let mut args = args.into_iter();
                if fname == "ensure_resource" {
                    let type_expr = args.next().unwrap_or(Expr::Undef);
                    let title_expr = args.next().unwrap_or(Expr::Undef);
                    let params_expr = args.next();
                    return Ok(Some(Stmt::EnsureResource {
                        type_expr,
                        title_expr,
                        params_expr,
                    }));
                }
                let packages_expr = args.next().unwrap_or(Expr::Undef);
                let params_expr = args.next();
                return Ok(Some(Stmt::EnsurePackages {
                    packages_expr,
                    params_expr,
                }));
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
                // A capitalized type reference followed by an attribute block
                // and no title is a resource default (`File { owner => ... }`),
                // not a declaration. Each namespace segment of a Puppet type
                // reference is capitalized, so the leading character decides.
                if is_type_reference(&rtype) {
                    let attrs = self.parse_attributes()?;
                    self.consume(TokenKind::RBrace);
                    return Ok(Some(Stmt::ResourceDefault { rtype, attrs }));
                }
                let titles = self.parse_titles()?;
                let attrs = self.parse_attributes()?;
                self.consume(TokenKind::RBrace);
                return Ok(Some(Stmt::Resource {
                    rtype,
                    titles,
                    attrs,
                }));
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
        while !self
            .peek_kind()
            .map_or(false, |kind| kind == TokenKind::RBrace)
            && !self.is_eof()
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
            if matches!(
                self.peek_kind(),
                Some(TokenKind::String) | Some(TokenKind::Var)
            ) {
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
            return Ok(Cond::Compare {
                left,
                op: CompareOp::Eq,
                right,
            });
        }
        if self.consume(TokenKind::NotEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::NotEq,
                right,
            });
        }
        if self.consume(TokenKind::Match) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::Match,
                right,
            });
        }
        if self.consume(TokenKind::NotMatch) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::NotMatch,
                right,
            });
        }
        if self.consume(TokenKind::LtEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::LtEq,
                right,
            });
        }
        if self.consume(TokenKind::GtEq) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::GtEq,
                right,
            });
        }
        if self.consume(TokenKind::Lt) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::Lt,
                right,
            });
        }
        if self.consume(TokenKind::Gt) {
            let right = self.parse_expr()?;
            return Ok(Cond::Compare {
                left,
                op: CompareOp::Gt,
                right,
            });
        }
        if self.consume_keyword("in") {
            let right = self.parse_expr()?;
            return Ok(Cond::In {
                needle: left,
                haystack: right,
            });
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
                // `.method |params| { body }` in value position is a lambda. `|`
                // is an unambiguous lambda signal — there's no hash/block
                // ambiguity to guard against — so attach it whenever it follows
                // a method postfix, regardless of the enclosing braces. (The
                // statement-`.each` detector disables this so it can read the
                // block as resource-declaring statements.)
                if self.lambda_in_expr && self.peek_kind() == Some(TokenKind::Pipe) {
                    let params = self.parse_lambda_params()?;
                    let body = self.parse_lambda_value_body()?;
                    expr = Expr::Lambda {
                        target: Box::new(expr),
                        method: name,
                        params,
                        body: Box::new(body),
                    };
                    continue;
                }
                expr = Expr::MethodCall {
                    target: Box::new(expr),
                    name,
                };
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
                expr = Expr::Arith {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                };
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
            let path = self.parse_index_path()?;
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
            // A capitalized data-type name (`String`, `Array`, `Optional`, …)
            // followed by no `(` is a type reference. The call form `Integer('7')`
            // is a conversion function and is handled above. Resource types like
            // `File` are *not* data types, so `File['/etc']` still parses as a
            // resource reference below.
            if is_data_type(&ident) {
                return self.parse_type_spec(ident);
            }
            if self.allow_bare_vars {
                let path = self.parse_index_path()?;
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

    /// Parse a chain of `[key]` index suffixes into index expressions. Each key
    /// is kept as an `Expr` and evaluated at resolve time, so `$h[$k]`, `$h[5]`,
    /// and `$arr[0]` all resolve dynamically rather than freezing to a literal.
    fn parse_index_path(&mut self) -> Result<Vec<Expr>> {
        let mut path = Vec::new();
        while self.consume(TokenKind::LBracket) {
            path.push(self.parse_expr()?);
            self.consume(TokenKind::RBracket);
        }
        Ok(path)
    }

    /// Parse a data type and its optional bracketed arguments, e.g.
    /// `String`, `String[1,10]`, `Optional[Array[String]]`, `Enum['a','b']`.
    fn parse_type_spec(&mut self, name: String) -> Result<Expr> {
        let mut args = Vec::new();
        if self.consume(TokenKind::LBracket) {
            while !self.consume(TokenKind::RBracket) && !self.is_eof() {
                if self.consume(TokenKind::Comma) {
                    continue;
                }
                args.push(self.parse_type_arg()?);
            }
        }
        Ok(Expr::Type(TypeSpec { name, args }))
    }

    /// Parse a single type argument: a numeric bound, the `default` keyword, a
    /// string/regex member, or a nested type.
    fn parse_type_arg(&mut self) -> Result<TypeArg> {
        if self.consume_keyword("default") {
            return Ok(TypeArg::Default);
        }
        if self.peek_kind() == Some(TokenKind::LBrace) {
            return self.parse_struct_fields();
        }
        match self.peek_kind() {
            Some(TokenKind::Number) => Ok(TypeArg::Int(self.expect_number()?)),
            Some(TokenKind::Minus) => {
                self.index += 1;
                Ok(TypeArg::Int(-self.expect_number()?))
            }
            Some(TokenKind::String) => Ok(TypeArg::Str(self.expect_string()?)),
            Some(TokenKind::Regex) => {
                let token = self.tokens[self.index].clone();
                self.index += 1;
                Ok(TypeArg::Regex(token.text))
            }
            Some(TokenKind::Ident) => {
                let ident = self.expect_ident()?;
                match self.parse_type_spec(ident)? {
                    Expr::Type(spec) => Ok(TypeArg::Type(spec)),
                    _ => unreachable!("parse_type_spec always returns Expr::Type"),
                }
            }
            // Unrecognized argument shape: consume one token to stay robust and
            // treat it as an unbounded slot rather than aborting the parse.
            _ => {
                self.index += 1;
                Ok(TypeArg::Default)
            }
        }
    }

    /// Parse the `{ Optional['k'] => <type>, 'k2' => <type>, … }` body of a
    /// `Struct` type into its field set.
    fn parse_struct_fields(&mut self) -> Result<TypeArg> {
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.consume(TokenKind::RBrace) && !self.is_eof() {
            if self.consume(TokenKind::Comma) {
                continue;
            }
            let Some((key, optional)) = self.parse_struct_key()? else {
                // Unrecognized key shape — skip a token to stay robust.
                self.index += 1;
                continue;
            };
            self.consume(TokenKind::FatArrow);
            let value = self.parse_type_value()?;
            fields.push(StructField {
                key,
                optional,
                value,
            });
            self.consume(TokenKind::Comma);
        }
        Ok(TypeArg::Struct(fields))
    }

    /// Parse a Struct key: a bare `'name'` string, or `Optional['name']` /
    /// `NotUndef['name']` (the latter marks the key required-and-not-undef; we
    /// model both wrappers' presence requirement, treating only `Optional` as
    /// truly optional).
    fn parse_struct_key(&mut self) -> Result<Option<(String, bool)>> {
        match self.peek_kind() {
            Some(TokenKind::String) => Ok(Some((self.expect_string()?, false))),
            Some(TokenKind::Ident) => {
                let ident = self.expect_ident()?;
                let optional = ident == "Optional";
                // Consume the `['name']` wrapper.
                if self.consume(TokenKind::LBracket) {
                    let name = self.expect_string().unwrap_or_default();
                    self.consume(TokenKind::RBracket);
                    Ok(Some((name, optional)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Parse a type used in value position (a Struct field's value, a Hash's
    /// value type, …). Any leading identifier is treated as a type name —
    /// including alias references like `Ssh::Yes_no` or `Stdlib::Port` — so it
    /// is not gated by [`is_data_type`].
    fn parse_type_value(&mut self) -> Result<TypeSpec> {
        if self.peek_kind() == Some(TokenKind::Ident) {
            let ident = self.expect_ident()?;
            if let Expr::Type(spec) = self.parse_type_spec(ident)? {
                return Ok(spec);
            }
        }
        // Fallback: an unrecognized value type matches leniently.
        Ok(TypeSpec {
            name: "Any".to_string(),
            args: Vec::new(),
        })
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

    /// Parse the argument(s) of an `include`/`contain`/`require` statement into a
    /// flat list of class names. Accepts a comma-separated list, optionally
    /// wrapped in parentheses, where each element is a bare identifier, a quoted
    /// string, or a bracketed array of those — each optionally carrying a legacy
    /// `::` prefix.
    fn parse_class_refs(&mut self) -> Result<Vec<String>> {
        let wrapped = self.consume(TokenKind::LParen);
        let mut names = Vec::new();
        loop {
            self.consume_leading_namespace_prefix();
            match self.peek_kind() {
                Some(TokenKind::Ident) => names.push(self.expect_ident()?),
                Some(TokenKind::String) => names.push(self.expect_string()?),
                Some(TokenKind::LBracket) => {
                    self.index += 1; // consume `[`
                    while !self.consume(TokenKind::RBracket) && !self.is_eof() {
                        if self.consume(TokenKind::Comma) {
                            continue;
                        }
                        self.consume_leading_namespace_prefix();
                        match self.peek_kind() {
                            Some(TokenKind::Ident) => names.push(self.expect_ident()?),
                            Some(TokenKind::String) => names.push(self.expect_string()?),
                            _ => {
                                self.index += 1;
                            }
                        }
                    }
                }
                _ => break,
            }
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }
        if wrapped {
            self.consume(TokenKind::RParen);
        }
        Ok(names)
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
            Expr::Lambda { method, .. } => method.clone(),
            Expr::ResourceRef { rtype, .. } => rtype.clone(),
            Expr::FunctionCall { name, .. } => name.clone(),
            Expr::Regex(pattern) => pattern.clone(),
            Expr::Selector { .. } => "selector".to_string(),
            Expr::Condition(_) => "condition".to_string(),
            Expr::Arith { .. } => "arith".to_string(),
            Expr::Type(spec) => spec.render(),
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

struct Lexer {
    chars: Vec<char>,
    index: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Self {
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
        self.chars[start..self.index].iter().collect()
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
        self.chars[start..self.index].iter().collect()
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
        self.chars[start..self.index].iter().collect()
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
    if op == ArithOp::Add {
        // Array `+`: Puppet concatenates two arrays, or appends a single
        // non-array element to the left-hand array.
        if let PuppetValue::Array(items) = left {
            let mut combined = items.clone();
            match right {
                PuppetValue::Array(more) => combined.extend(more.iter().cloned()),
                other => combined.push(other.clone()),
            }
            return PuppetValue::Array(combined);
        }
        // Hash `+`: merge the right-hand entries over a copy of the left.
        if let (PuppetValue::Hash(base), PuppetValue::Hash(extra)) = (left, right) {
            let mut merged = base.clone();
            for (key, value) in extra {
                merged.insert(key.clone(), value.clone());
            }
            return PuppetValue::Hash(merged);
        }
    }
    // Hash-minus (`$h - 'key'`) and other non-integer combinations: keep the
    // left-hand value so downstream code still sees something sensible.
    left.clone()
}

/// Coerce a value to an integer the way Puppet's `Integer()` function does:
/// integers pass through, numeric strings parse (with optional sign and
/// `0x`/`0o`/`0b` radix prefixes). Returns `None` when the value isn't a whole
/// number, so callers can fall back rather than fabricate a `0`.
fn coerce_to_integer(value: &PuppetValue) -> Option<i64> {
    match value {
        PuppetValue::Integer(n) => Some(*n),
        PuppetValue::String(s) => {
            let trimmed = s.trim();
            let (sign, digits) = match trimmed.strip_prefix('-') {
                Some(rest) => (-1, rest),
                None => (1, trimmed.strip_prefix('+').unwrap_or(trimmed)),
            };
            let magnitude = if let Some(hex) = digits
                .strip_prefix("0x")
                .or_else(|| digits.strip_prefix("0X"))
            {
                i64::from_str_radix(hex, 16).ok()
            } else if let Some(oct) = digits
                .strip_prefix("0o")
                .or_else(|| digits.strip_prefix("0O"))
            {
                i64::from_str_radix(oct, 8).ok()
            } else if let Some(bin) = digits
                .strip_prefix("0b")
                .or_else(|| digits.strip_prefix("0B"))
            {
                i64::from_str_radix(bin, 2).ok()
            } else {
                digits.parse::<i64>().ok()
            };
            magnitude.map(|m| sign * m)
        }
        _ => None,
    }
}

fn eval_ordered_compare(left: &PuppetValue, op: CompareOp, right: &PuppetValue) -> bool {
    let ordering = match (coerce_to_integer(left), coerce_to_integer(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        // Only fall back to lexical comparison when *neither* side is numeric.
        // A mixed numeric/non-numeric comparison (e.g. `7 >= 'undef'`) has no
        // meaningful lexical answer — comparing the strings `"7"` and `"undef"`
        // would give a bogus result — so treat it as not-ordered.
        (None, None) => left.as_string().cmp(&right.as_string()),
        _ => return false,
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
    let re = Regex::new(&pattern).with_context(|| format!("invalid regex pattern: /{pattern}/"))?;
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
    let col = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.len())
        .unwrap_or(prefix.len())
        + 1;
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
        PuppetValue::Hash(m) => m.clone().into_iter().collect(),
        _ => HashMap::new(),
    };
    let mut idx = 0;
    render_epp_block(&tokens, &mut idx, &params_map, true)
}

// ---------------------------------------------------------------------------
// ERB templates (`template()` / `inline_template()`)
// ---------------------------------------------------------------------------

/// A lexed ERB fragment.
enum ErbToken {
    Text(String),
    /// `<%= EXPR %>` — output the expression.
    Output(String),
    /// `<% CODE %>` — control flow (`if`/`unless`/`each`/`else`/`end`).
    Code(String),
}

/// A parsed ERB node tree.
enum ErbNode {
    Text(String),
    Output(String),
    If {
        cond: String,
        negate: bool,
        body: Vec<ErbNode>,
        else_body: Vec<ErbNode>,
    },
    Each {
        iterable: String,
        var: String,
        body: Vec<ErbNode>,
    },
}

/// Render the subset of ERB that Puppet `template()` files use in practice:
/// `<% if/unless EXPR -%>` … `<% else -%>` … `<% end -%>`, `<% ARR.each do |v|
/// -%>` … `<% end -%>`, `<%= EXPR %>` output, and `-%>`/`<%-` whitespace
/// trimming. Instance variables (`@x`) resolve against the Puppet `scope`.
fn render_erb(template: &str, scope: &HashMap<String, PuppetValue>) -> String {
    let tokens = tokenize_erb(template);
    let mut idx = 0;
    let nodes = parse_erb_nodes(&tokens, &mut idx);
    let mut out = String::new();
    render_erb_nodes(&nodes, scope, &mut out);
    out
}

fn tokenize_erb(template: &str) -> Vec<ErbToken> {
    let chars: Vec<char> = template.chars().collect();
    let mut tokens: Vec<ErbToken> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' && chars.get(i + 1) == Some(&'%') {
            // `<%-` trims trailing whitespace of the preceding text line.
            let lead_trim = chars.get(i + 2) == Some(&'-');
            if lead_trim {
                while text.ends_with(' ') || text.ends_with('\t') {
                    text.pop();
                }
            }
            tokens.push(ErbToken::Text(std::mem::take(&mut text)));
            let is_output = chars.get(i + 2) == Some(&'=');
            let mut j = i + 2;
            // Find the closing `%>`.
            while j < chars.len() && !(chars[j] == '%' && chars.get(j + 1) == Some(&'>')) {
                j += 1;
            }
            let mut code: String = chars[i + 2..j.min(chars.len())].iter().collect();
            // A trailing `-` (`-%>`) trims the newline that follows the tag.
            let trail_trim = code.ends_with('-');
            if trail_trim {
                code.pop();
            }
            if is_output {
                code = code.trim_start_matches('=').to_string();
                tokens.push(ErbToken::Output(code.trim().to_string()));
            } else {
                let trimmed = code.trim_start_matches('-').trim().to_string();
                tokens.push(ErbToken::Code(trimmed));
            }
            i = j + 2; // skip past `%>`
            if trail_trim {
                // Drop a single immediately-following newline.
                if chars.get(i) == Some(&'\n') {
                    i += 1;
                } else if chars.get(i) == Some(&'\r') && chars.get(i + 1) == Some(&'\n') {
                    i += 2;
                }
            }
        } else {
            text.push(chars[i]);
            i += 1;
        }
    }
    tokens.push(ErbToken::Text(text));
    tokens
}

/// Parse a flat token stream into nodes until a block terminator (`end`,
/// `else`, `elsif`) or end of stream. `*idx` is left pointing at the terminator
/// code token (if any) so the caller can branch on it.
fn parse_erb_nodes(tokens: &[ErbToken], idx: &mut usize) -> Vec<ErbNode> {
    let mut nodes = Vec::new();
    while *idx < tokens.len() {
        match &tokens[*idx] {
            ErbToken::Text(t) => {
                if !t.is_empty() {
                    nodes.push(ErbNode::Text(t.clone()));
                }
                *idx += 1;
            }
            ErbToken::Output(code) => {
                nodes.push(ErbNode::Output(code.clone()));
                *idx += 1;
            }
            ErbToken::Code(code) => {
                let lower = code.trim();
                if lower == "end" || lower == "else" || lower.starts_with("elsif") {
                    // Terminator for the enclosing block — stop, leave *idx here.
                    return nodes;
                }
                if let Some(rest) = lower
                    .strip_prefix("if ")
                    .map(|r| (r, false))
                    .or_else(|| lower.strip_prefix("unless ").map(|r| (r, true)))
                {
                    let (cond, negate) = rest;
                    let cond = cond.to_string();
                    *idx += 1;
                    let body = parse_erb_nodes(tokens, idx);
                    // Handle else/elsif chains.
                    let mut else_body = Vec::new();
                    if let Some(ErbToken::Code(term)) = tokens.get(*idx) {
                        let term = term.trim().to_string();
                        if term == "else" {
                            *idx += 1;
                            else_body = parse_erb_nodes(tokens, idx);
                        } else if let Some(elsif_cond) = term.strip_prefix("elsif ") {
                            // Represent `elsif` as a nested if in the else branch.
                            let nested = ErbNode::If {
                                cond: elsif_cond.to_string(),
                                negate: false,
                                body: {
                                    *idx += 1;
                                    parse_erb_nodes(tokens, idx)
                                },
                                else_body: Vec::new(),
                            };
                            else_body = vec![nested];
                        }
                    }
                    // Consume the matching `end`.
                    if matches!(tokens.get(*idx), Some(ErbToken::Code(t)) if t.trim() == "end") {
                        *idx += 1;
                    }
                    nodes.push(ErbNode::If {
                        cond,
                        negate,
                        body,
                        else_body,
                    });
                } else if let Some((iterable, var)) = parse_erb_each(lower) {
                    *idx += 1;
                    let body = parse_erb_nodes(tokens, idx);
                    if matches!(tokens.get(*idx), Some(ErbToken::Code(t)) if t.trim() == "end") {
                        *idx += 1;
                    }
                    nodes.push(ErbNode::Each {
                        iterable,
                        var,
                        body,
                    });
                } else {
                    // Unsupported code (assignment, comment, …): ignore.
                    *idx += 1;
                }
            }
        }
    }
    nodes
}

/// Parse `<iterable>.each do |<var>|` → (iterable, var).
fn parse_erb_each(code: &str) -> Option<(String, String)> {
    let dot = code.find(".each")?;
    let iterable = code[..dot].trim().to_string();
    let bar_open = code.find('|')?;
    let bar_close = code[bar_open + 1..].find('|')? + bar_open + 1;
    let var = code[bar_open + 1..bar_close].trim().to_string();
    if iterable.is_empty() || var.is_empty() {
        return None;
    }
    Some((iterable, var))
}

fn render_erb_nodes(nodes: &[ErbNode], scope: &HashMap<String, PuppetValue>, out: &mut String) {
    for node in nodes {
        match node {
            ErbNode::Text(t) => out.push_str(t),
            ErbNode::Output(code) => {
                let value = erb_eval_expr(code, scope);
                if !matches!(value, PuppetValue::Undef) {
                    out.push_str(&value.as_string());
                }
            }
            ErbNode::If {
                cond,
                negate,
                body,
                else_body,
            } => {
                let truthy = erb_eval_cond(cond, scope) ^ negate;
                if truthy {
                    render_erb_nodes(body, scope, out);
                } else {
                    render_erb_nodes(else_body, scope, out);
                }
            }
            ErbNode::Each {
                iterable,
                var,
                body,
            } => {
                if let PuppetValue::Array(items) = erb_eval_expr(iterable, scope) {
                    for item in items {
                        let mut inner = scope.clone();
                        inner.insert(var.clone(), item);
                        render_erb_nodes(body, &inner, out);
                    }
                }
            }
        }
    }
}

/// Resolve an ERB variable reference (`@name` or a bare loop variable `name`)
/// against the scope.
fn erb_lookup(name: &str, scope: &HashMap<String, PuppetValue>) -> PuppetValue {
    let key = name.trim().trim_start_matches('@');
    scope.get(key).cloned().unwrap_or(PuppetValue::Undef)
}

/// Evaluate an ERB output/iterable expression: a variable, or `VAR.join('sep')`.
fn erb_eval_expr(code: &str, scope: &HashMap<String, PuppetValue>) -> PuppetValue {
    let code = code.trim();
    if let Some(dot) = code.find(".join(") {
        let base = &code[..dot];
        let after = &code[dot + ".join(".len()..];
        let sep = after
            .rsplit_once(')')
            .map(|(inner, _)| inner)
            .unwrap_or(after)
            .trim()
            .trim_matches(|c| c == '\'' || c == '"');
        if let PuppetValue::Array(items) = erb_lookup(base, scope) {
            let joined = items
                .iter()
                .map(|v| v.as_string())
                .collect::<Vec<_>>()
                .join(sep);
            return PuppetValue::String(joined);
        }
        return PuppetValue::Undef;
    }
    erb_lookup(code, scope)
}

/// Evaluate an ERB `if`/`unless` condition. Supports `EXPR != nil`, `EXPR ==
/// nil`, and a bare truthiness test.
fn erb_eval_cond(code: &str, scope: &HashMap<String, PuppetValue>) -> bool {
    let code = code.trim();
    if let Some(base) = code.strip_suffix("!= nil").map(str::trim) {
        return !matches!(erb_lookup(base, scope), PuppetValue::Undef);
    }
    if let Some(base) = code.strip_suffix("== nil").map(str::trim) {
        return matches!(erb_lookup(base, scope), PuppetValue::Undef);
    }
    match erb_eval_expr(code, scope) {
        PuppetValue::Undef => false,
        PuppetValue::Bool(b) => b,
        _ => true,
    }
}

/// Render tokens starting at `*idx` until the closing `}` of the current block
/// (or end of stream). On return, `*idx` points at the code token that closed
/// the block (`}`, `} else …`, `} elsif …`) so the caller can decide what to
/// do next; for a plain `}` the caller is responsible for consuming it.
/// `emit` is false while rendering a branch that should not produce output.
fn render_epp_block(
    tokens: &[EppToken],
    idx: &mut usize,
    params: &HashMap<String, PuppetValue>,
    emit: bool,
) -> String {
    let mut out = String::new();
    while *idx < tokens.len() {
        match &tokens[*idx] {
            EppToken::Text(text) => {
                if emit {
                    out.push_str(text);
                }
                *idx += 1;
            }
            EppToken::Output(expr) => {
                if emit {
                    out.push_str(&epp_eval(expr.trim(), params).as_string());
                }
                *idx += 1;
            }
            EppToken::Code(code) => {
                let trimmed = code.trim();
                // Parameter header (`<%- | params | -%>`).
                if trimmed.starts_with('|') {
                    *idx += 1;
                    continue;
                }
                // A closer for the enclosing block — hand control back.
                if trimmed == "}" || trimmed.starts_with("} else") || trimmed.starts_with("} elsif")
                {
                    return out;
                }
                if parse_each_header(trimmed).is_some() {
                    out.push_str(&render_epp_each(tokens, idx, params, emit, trimmed));
                    continue;
                }
                if trimmed.starts_with("if ") || trimmed.starts_with("unless ") {
                    out.push_str(&render_epp_if(tokens, idx, params, emit, trimmed));
                    continue;
                }
                // Unknown code tag — ignore (no error, mirrors tolerant style
                // used elsewhere in the evaluator).
                *idx += 1;
            }
        }
    }
    out
}

/// Render an `if`/`unless` chain. `*idx` points at the opening code token;
/// on return it has been advanced past the closing `}`.
fn render_epp_if(
    tokens: &[EppToken],
    idx: &mut usize,
    params: &HashMap<String, PuppetValue>,
    parent_emit: bool,
    opener: &str,
) -> String {
    let (cond_src, negate) = if let Some(rest) = opener.strip_prefix("if ") {
        (rest.trim_end_matches('{').trim(), false)
    } else {
        (opener["unless ".len()..].trim_end_matches('{').trim(), true)
    };
    let mut matched = epp_truthy(&epp_eval(cond_src, params));
    if negate {
        matched = !matched;
    }
    let mut active = parent_emit && matched;
    let mut branch_taken = active;
    let mut out = String::new();
    *idx += 1; // consume the opener
    loop {
        out.push_str(&render_epp_block(tokens, idx, params, active));
        if *idx >= tokens.len() {
            break;
        }
        let closer = match &tokens[*idx] {
            EppToken::Code(c) => c.trim().to_string(),
            _ => break,
        };
        if closer == "}" {
            *idx += 1;
            break;
        } else if let Some(rest) = closer.strip_prefix("} elsif ") {
            *idx += 1;
            let cond = rest.trim_end_matches('{').trim();
            let take = parent_emit && !branch_taken && epp_truthy(&epp_eval(cond, params));
            active = take;
            branch_taken = branch_taken || take;
        } else if closer.starts_with("} else") {
            *idx += 1;
            active = parent_emit && !branch_taken;
            branch_taken = true;
        } else {
            break;
        }
    }
    out
}

/// Render an `<arr>.each |…| { … }` loop. `*idx` points at the opening code
/// token; on return it has been advanced past the closing `}`.
fn render_epp_each(
    tokens: &[EppToken],
    idx: &mut usize,
    params: &HashMap<String, PuppetValue>,
    emit: bool,
    header: &str,
) -> String {
    let (collection_expr, vars) = parse_each_header(header).expect("caller verified header");
    *idx += 1; // consume the opener
    let body_end = find_epp_block_end(tokens, *idx);
    let body = &tokens[*idx..body_end];
    // Advance past the body and its closing `}` (if present).
    *idx = (body_end + 1).min(tokens.len());

    if !emit {
        return String::new();
    }
    let mut out = String::new();
    match epp_eval(&collection_expr, params) {
        PuppetValue::Array(items) => {
            for item in items {
                let mut scope = params.clone();
                bind_each_vars(&mut scope, &vars, &[item]);
                let mut bidx = 0;
                out.push_str(&render_epp_block(body, &mut bidx, &scope, true));
            }
        }
        PuppetValue::Hash(entries) => {
            // PuppetValue::Hash is unordered; iterate by sorted key so rendered
            // output is deterministic (modules build these data hashes in key
            // order, so this reproduces the expected file content).
            // PuppetValue::Hash preserves insertion order (IndexMap), so iterate
            // as-is — this reproduces the key order Puppet renders.
            for (key, value) in entries {
                let mut scope = params.clone();
                if vars.len() == 1 {
                    scope.insert(
                        vars[0].clone(),
                        PuppetValue::Array(vec![PuppetValue::String(key), value]),
                    );
                } else {
                    bind_each_vars(&mut scope, &vars, &[PuppetValue::String(key), value]);
                }
                let mut bidx = 0;
                out.push_str(&render_epp_block(body, &mut bidx, &scope, true));
            }
        }
        _ => {}
    }
    out
}

fn bind_each_vars(
    scope: &mut HashMap<String, PuppetValue>,
    vars: &[String],
    values: &[PuppetValue],
) {
    for (name, value) in vars.iter().zip(values.iter()) {
        scope.insert(name.clone(), value.clone());
    }
}

/// Parse an `.each` header such as `$items.each |$item| {` or
/// `$h.each |$k, $v| {`, returning the collection expression and the loop
/// variable names (without the leading `$`). Returns `None` if `trimmed` is
/// not an `.each` opener.
fn parse_each_header(trimmed: &str) -> Option<(String, Vec<String>)> {
    if !trimmed.ends_with('{') {
        return None;
    }
    let body = trimmed[..trimmed.len() - 1].trim();
    let (collection, rest) = body.split_once(".each")?;
    let collection = collection.trim();
    if collection.is_empty() {
        return None;
    }
    let rest = rest.trim();
    let inner = rest.strip_prefix('|')?.strip_suffix('|')?;
    let vars = inner
        .split(',')
        .map(|v| v.trim().trim_start_matches('$').to_string())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    if vars.is_empty() {
        return None;
    }
    Some((collection.to_string(), vars))
}

/// Given `start` pointing just past a block opener, return the index of the
/// matching closing `}` (or `tokens.len()` if unterminated). `} else {` and
/// `} elsif … {` are net-zero for nesting since they close and reopen.
fn find_epp_block_end(tokens: &[EppToken], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while i < tokens.len() {
        if let EppToken::Code(code) = &tokens[i] {
            let t = code.trim();
            if t == "}" {
                if depth == 0 {
                    return i;
                }
                depth -= 1;
            } else if t.starts_with('}') && t.ends_with('{') {
                // `} else {` / `} elsif … {`: closes one and opens one.
            } else if t.ends_with('{') {
                depth += 1;
            }
        }
        i += 1;
    }
    i
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

/// Evaluate an EPP expression. Supports logical (`and`/`or`/`!`/`not`),
/// comparison (`==`, `!=`, `<`, `>`, `<=`, `>=`) and parenthesised forms on top
/// of the primitive `$var`/literal lookups in [`epp_eval_primary`]. This is what
/// lets `<% if $x != '' { %>` and friends render faithfully — without it a
/// condition like `$x != ''` was treated as a single (missing) variable name and
/// always evaluated falsy, silently dropping the whole branch.
fn epp_eval(source: &str, params: &HashMap<String, PuppetValue>) -> PuppetValue {
    let s = source.trim();
    // Lowest precedence first: `or`, then `and`.
    if let Some((l, _, r)) = epp_split_binary(s, &["or"], true) {
        return PuppetValue::Bool(
            epp_truthy(&epp_eval(&l, params)) || epp_truthy(&epp_eval(&r, params)),
        );
    }
    if let Some((l, _, r)) = epp_split_binary(s, &["and"], true) {
        return PuppetValue::Bool(
            epp_truthy(&epp_eval(&l, params)) && epp_truthy(&epp_eval(&r, params)),
        );
    }
    // Comparisons (two-char operators listed before one-char so `<=`/`>=` win).
    if let Some((l, op, r)) = epp_split_binary(s, &["==", "!=", "<=", ">=", "<", ">"], false) {
        return PuppetValue::Bool(epp_compare(
            &epp_eval(&l, params),
            &op,
            &epp_eval(&r, params),
        ));
    }
    // Unary negation.
    if let Some(rest) = s.strip_prefix('!') {
        return PuppetValue::Bool(!epp_truthy(&epp_eval(rest, params)));
    }
    if let Some(rest) = s.strip_prefix("not ") {
        return PuppetValue::Bool(!epp_truthy(&epp_eval(rest, params)));
    }
    // Fully parenthesised expression.
    if epp_is_paren_wrapped(s) {
        return epp_eval(&s[1..s.len() - 1], params);
    }
    epp_eval_primary(s, params)
}

/// Evaluate a primitive EPP expression: a `$var` reference, string/integer
/// literal, or boolean/undef keyword.
fn epp_eval_primary(source: &str, params: &HashMap<String, PuppetValue>) -> PuppetValue {
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

/// Split `s` on the first top-level occurrence of any operator in `ops`,
/// ignoring matches inside quotes or parentheses. When `word` is true the
/// operator must be surrounded by whitespace (for keyword operators like
/// `and`/`or`). Returns `(left, op, right)`.
fn epp_split_binary(s: &str, ops: &[&str], word: bool) -> Option<(String, String, String)> {
    let chars: Vec<char> = s.chars().collect();
    let mut depth: i32 = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' => {
                in_single = true;
                i += 1;
                continue;
            }
            '"' => {
                in_double = true;
                i += 1;
                continue;
            }
            '(' => {
                depth += 1;
                i += 1;
                continue;
            }
            ')' => {
                depth -= 1;
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0 {
            for op in ops {
                let op_chars: Vec<char> = op.chars().collect();
                if chars[i..].starts_with(&op_chars[..]) {
                    if word {
                        let before_ok = i == 0 || chars[i - 1].is_whitespace();
                        let after = i + op_chars.len();
                        let after_ok = after >= chars.len() || chars[after].is_whitespace();
                        if !(before_ok && after_ok) {
                            continue;
                        }
                    }
                    let left: String = chars[..i].iter().collect();
                    let right: String = chars[i + op_chars.len()..].iter().collect();
                    return Some((left, (*op).to_string(), right));
                }
            }
        }
        i += 1;
    }
    None
}

/// True when `s` is a single parenthesised group spanning the whole string,
/// e.g. `($a and $b)` but not `($a) or ($b)`.
fn epp_is_paren_wrapped(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.first() != Some(&'(') || chars.last() != Some(&')') {
        return false;
    }
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    for (i, &c) in chars.iter().enumerate() {
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            }
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                // If we close the opening paren before the final char, the
                // outer parens don't wrap the whole expression.
                if depth == 0 && i != chars.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Compare two EPP values for an `if` condition.
fn epp_compare(left: &PuppetValue, op: &str, right: &PuppetValue) -> bool {
    match op {
        "==" => epp_values_eq(left, right),
        "!=" => !epp_values_eq(left, right),
        "<" | ">" | "<=" | ">=" => match epp_compare_ord(left, right) {
            Some(ord) => match op {
                "<" => ord == std::cmp::Ordering::Less,
                ">" => ord == std::cmp::Ordering::Greater,
                "<=" => ord != std::cmp::Ordering::Greater,
                ">=" => ord != std::cmp::Ordering::Less,
                _ => false,
            },
            None => false,
        },
        _ => false,
    }
}

fn epp_values_eq(a: &PuppetValue, b: &PuppetValue) -> bool {
    match (a, b) {
        (PuppetValue::Integer(x), PuppetValue::Integer(y)) => x == y,
        (PuppetValue::Bool(x), PuppetValue::Bool(y)) => x == y,
        (PuppetValue::Undef, PuppetValue::Undef) => true,
        (PuppetValue::String(x), PuppetValue::String(y)) => x == y,
        _ => a.as_string() == b.as_string(),
    }
}

fn epp_compare_ord(a: &PuppetValue, b: &PuppetValue) -> Option<std::cmp::Ordering> {
    if let (PuppetValue::Integer(x), PuppetValue::Integer(y)) = (a, b) {
        return Some(x.cmp(y));
    }
    // Fall back to numeric comparison when both stringify to numbers,
    // otherwise lexicographic — mirroring Puppet's permissive comparisons.
    let (sa, sb) = (a.as_string(), b.as_string());
    if let (Ok(x), Ok(y)) = (sa.parse::<f64>(), sb.parse::<f64>()) {
        return x.partial_cmp(&y);
    }
    Some(sa.cmp(&sb))
}

fn epp_truthy(value: &PuppetValue) -> bool {
    match value {
        PuppetValue::Bool(b) => *b,
        PuppetValue::Undef => false,
        PuppetValue::String(s) => !s.is_empty(),
        PuppetValue::Array(a) => !a.is_empty(),
        PuppetValue::Hash(h) => !h.is_empty(),
        PuppetValue::Integer(_) => true,
        PuppetValue::Float(_) => true,
    }
}

/// Render a float the way Puppet does (drop a trailing `.0` only when the value
/// is integral is *not* Puppet's behavior — it always shows a decimal — so keep
/// the natural Rust formatting, which yields `2.42`, `0.5`, `3` → `3`).
fn format_puppet_float(value: f64) -> String {
    if value.fract() == 0.0 && value.is_finite() {
        format!("{value:.1}")
    } else {
        value.to_string()
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
    fn type_aliases_drive_allow_value_matching() {
        let dir = tempfile::tempdir().unwrap();
        let types = dir.path().join("types");
        fs::create_dir_all(&types).unwrap();
        fs::write(
            types.join("yes_no.pp"),
            "type Ssh::Yes_no = Enum['yes', 'no']\n",
        )
        .unwrap();
        // A Struct that references the alias and uses Optional keys with a
        // String-valued field (which exercises the undef quirk) plus an
        // Enum-valued field (which must reject undef).
        fs::write(
            types.join("cfg.pp"),
            "type Ssh::Cfg = Struct[{\n  Optional['Name'] => String[1],\n  Optional['Flag'] => Ssh::Yes_no,\n}]\n",
        )
        .unwrap();
        let ev = PuppetEvaluator::new(dir.path()).unwrap();

        let s = |v: &str| PuppetValue::String(v.to_string());
        // Enum alias.
        assert_eq!(ev.type_allows("Ssh::Yes_no", &s("yes")), Some(true));
        assert_eq!(ev.type_allows("Ssh::Yes_no", &s("maybe")), Some(false));
        assert_eq!(
            ev.type_allows("Ssh::Yes_no", &PuppetValue::Undef),
            Some(false)
        );
        // Unknown type -> None so the caller can report it.
        assert_eq!(ev.type_allows("Ssh::Nope", &s("x")), None);

        // Struct single-key hashes.
        let hash = |k: &str, v: PuppetValue| {
            PuppetValue::Hash(IndexMap::from([(k.to_string(), v)]))
        };
        // Valid Enum field value.
        assert_eq!(ev.type_allows("Ssh::Cfg", &hash("Flag", s("no"))), Some(true));
        // Invalid Enum field value, and undef rejected for an Enum field.
        assert_eq!(ev.type_allows("Ssh::Cfg", &hash("Flag", s("x"))), Some(false));
        assert_eq!(
            ev.type_allows("Ssh::Cfg", &hash("Flag", PuppetValue::Undef)),
            Some(false)
        );
        // String field: a value matches, and undef is accepted (the Puppet
        // quirk for String-typed optional struct keys).
        assert_eq!(ev.type_allows("Ssh::Cfg", &hash("Name", s("eth0"))), Some(true));
        assert_eq!(
            ev.type_allows("Ssh::Cfg", &hash("Name", PuppetValue::Undef)),
            Some(true)
        );
        // Unknown struct key is rejected.
        assert_eq!(ev.type_allows("Ssh::Cfg", &hash("Bogus", s("x"))), Some(false));
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
        let mut parser = PuppetParser::new("class foo {\n  include ::stdlib\n}\n");
        let defs = parser.parse_definitions().expect("parse");
        assert_eq!(defs.len(), 1);
        let warnings: Vec<String> = parser.warnings.iter().map(|w| w.message.clone()).collect();
        assert_eq!(warnings.len(), 1, "expected one deprecation warning");
        assert!(warnings[0].contains("leading `::`"));
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::Include(names)] => assert_eq!(names, &["stdlib".to_string()]),
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
        let mut parser =
            PuppetParser::new("class foo {\n  ::apache::vhost { 'site': port => 80 }\n}\n");
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
    fn parser_recognizes_resource_default_with_attributes() {
        // `File { ... }` — capitalized type, no title — is a resource default,
        // not a declaration. It must parse into a distinct statement form
        // rather than tripping the title parser looking for a `:`.
        let mut parser = PuppetParser::new(
            "class foo {\n  File {\n    owner => 'root',\n    mode  => '0644',\n  }\n}\n",
        );
        let defs = parser.parse_definitions().expect("parse");
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::ResourceDefault { rtype, attrs }] => {
                    assert_eq!(rtype, "File");
                    assert_eq!(attrs.len(), 2);
                    assert!(attrs.contains_key("owner"));
                    assert!(attrs.contains_key("mode"));
                }
                other => panic!("unexpected body: {other:?}"),
            },
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn parser_distinguishes_default_from_declaration_for_namespaced_type() {
        // A capitalized namespaced reference is a default; the lowercase form
        // is a normal declaration with a title.
        let mut parser = PuppetParser::new(
            "class foo {\n  Apache::Vhost { mode => '0644' }\n  apache::vhost { 'site': port => 80 }\n}\n",
        );
        let defs = parser.parse_definitions().expect("parse");
        match &defs[0] {
            PuppetDef::Class(c) => match &c.body[..] {
                [Stmt::ResourceDefault { rtype, .. }, Stmt::Resource {
                    rtype: decl,
                    titles,
                    ..
                }] => {
                    assert_eq!(rtype, "Apache::Vhost");
                    assert_eq!(decl, "apache::vhost");
                    assert_eq!(titles.len(), 1);
                }
                other => panic!("unexpected body: {other:?}"),
            },
            _ => panic!("expected class def"),
        }
    }

    #[test]
    fn resource_default_attributes_are_inherited_by_declarations() {
        // Defaults fill in attributes a later declaration does not set, but
        // never override ones it does set.
        let manifest = r#"
            class foo {
              File {
                owner => 'root',
                mode  => '0644',
              }
              file { '/x':
                mode => '0600',
              }
              file { '/y':
                ensure => file,
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("resource default must parse and evaluate");

        let x = catalog.find("file", "/x").expect("file[/x] present");
        // Declaration's own value wins over the default.
        assert_eq!(
            x.attributes.get("mode"),
            Some(&PuppetValue::String("0600".to_string()))
        );
        // Default fills in the attribute the declaration omitted.
        assert_eq!(
            x.attributes.get("owner"),
            Some(&PuppetValue::String("root".to_string()))
        );

        let y = catalog.find("file", "/y").expect("file[/y] present");
        assert_eq!(
            y.attributes.get("owner"),
            Some(&PuppetValue::String("root".to_string()))
        );
        assert_eq!(
            y.attributes.get("mode"),
            Some(&PuppetValue::String("0644".to_string()))
        );
    }

    #[test]
    fn parser_does_not_warn_when_no_leading_prefix_present() {
        let mut parser = PuppetParser::new("class foo {\n  include stdlib\n}\n");
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
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("manifest with escaped quotes and embedded braces must parse");
        assert!(catalog.contains("notify", "{\"k\": \"v\", \"nested\": {\"a\": 1}}"));
    }

    #[test]
    fn integer_conversion_compares_numerically_not_lexically() {
        // `Integer('7')` must yield the integer 7, and `7 >= 10` must be false.
        // The old bug returned Undef from `Integer(...)` and then compared
        // `"undef"` against `"10"` lexically, evaluating the guard as true.
        let manifest = r#"
            class foo {
              if Integer('7') >= 10 {
                notify { 'ten_or_more': }
              } else {
                notify { 'below_ten': }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("Integer() comparison must parse and evaluate");
        assert!(
            catalog.contains("notify", "below_ten"),
            "Integer('7') >= 10 must be false"
        );
        assert!(
            !catalog.contains("notify", "ten_or_more"),
            "Integer('7') >= 10 must not be true"
        );
    }

    #[test]
    fn integer_conversion_true_branch_when_threshold_met() {
        // Sanity check the conversion the other way: `Integer('12') >= 10`.
        let manifest = r#"
            class foo {
              if Integer('12') >= 10 {
                notify { 'ten_or_more': }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("Integer() comparison must parse and evaluate");
        assert!(catalog.contains("notify", "ten_or_more"));
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
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
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
        let mut facts = IndexMap::new();
        facts.insert(
            "osfamily".to_string(),
            PuppetValue::String("RedHat".to_string()),
        );
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
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
        let mut facts = IndexMap::new();
        facts.insert(
            "osfamily".to_string(),
            PuppetValue::String("RedHat".to_string()),
        );
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
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
        let mut facts = IndexMap::new();
        facts.insert(
            "osfamily".to_string(),
            PuppetValue::String("RedHat".to_string()),
        );
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
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
        let mut facts = IndexMap::new();
        facts.insert(
            "osfamily".to_string(),
            PuppetValue::String("RedHat".to_string()),
        );
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(facts),
            )
            .expect("manifest must parse");
        assert!(!catalog.contains("file", "/etc/should-not-exist"));
    }

    fn eval_with_osfamily(manifest: &str, osfamily: &str) -> PuppetCatalog {
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut facts = IndexMap::new();
        facts.insert(
            "osfamily".to_string(),
            PuppetValue::String(osfamily.to_string()),
        );
        evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
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
        std::fs::write(
            manifests.join("init.pp"),
            "class mymod { include mymod::foo }",
        )
        .unwrap();
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
            .evaluate_class(
                "mymod",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("evaluating mymod (which includes mymod::foo) must succeed");
        assert!(catalog.contains("file", "/etc/sibling-loaded"));
    }

    #[test]
    fn contain_declares_class_and_expands_body() {
        // `contain` must behave like `include`: declare the class into the
        // catalog and expand its body. Previously `contain` fell through to the
        // resource parser and was silently dropped.
        let manifest = r#"
            class baseapp {
              contain baseapp::config
            }
            class baseapp::config {
              file { '/etc/baseapp.conf': ensure => file }
            }
        "#;
        let dir = write_module("baseapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "baseapp",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with `contain` must evaluate");
        assert!(catalog.contains("class", "baseapp::config"));
        assert!(catalog.contains("file", "/etc/baseapp.conf"));
    }

    #[test]
    fn include_accepts_comma_separated_and_quoted_class_lists() {
        // `include a, b` and quoted/array forms must each declare every class.
        let manifest = r#"
            class baseapp {
              include baseapp::a, 'baseapp::b'
              contain ['baseapp::c']
            }
            class baseapp::a { file { '/etc/a': ensure => file } }
            class baseapp::b { file { '/etc/b': ensure => file } }
            class baseapp::c { file { '/etc/c': ensure => file } }
        "#;
        let dir = write_module("baseapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "baseapp",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with comma/array include forms must evaluate");
        for (class, file) in [
            ("baseapp::a", "/etc/a"),
            ("baseapp::b", "/etc/b"),
            ("baseapp::c", "/etc/c"),
        ] {
            assert!(catalog.contains("class", class), "missing class {class}");
            assert!(catalog.contains("file", file), "missing file {file}");
        }
    }

    #[test]
    fn loader_picks_up_nested_manifest_files() {
        // `mod::sub::leaf` should autoload from manifests/sub/leaf.pp.
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        std::fs::create_dir_all(manifests.join("sub")).unwrap();
        std::fs::write(
            manifests.join("init.pp"),
            "class mymod { include mymod::sub::leaf }",
        )
        .unwrap();
        std::fs::write(
            manifests.join("sub").join("leaf.pp"),
            "class mymod::sub::leaf { file { '/etc/nested-loaded': ensure => file } }",
        )
        .unwrap();

        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        assert!(
            evaluator
                .class_names()
                .iter()
                .any(|n| n == "mymod::sub::leaf"),
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
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with epp(...) followed by trailing comma must parse");
        assert!(catalog.contains("file", "/y"));
    }

    #[test]
    fn epp_each_iterates_array_binding_loop_var() {
        let template = "<% $items.each |$i| { -%>\n- <%= $i %>\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([(
            "items".to_string(),
            PuppetValue::Array(vec![
                PuppetValue::String("a".to_string()),
                PuppetValue::String("b".to_string()),
            ]),
        )]));
        assert_eq!(render_epp(template, &params), "- a\n- b\n");
    }

    #[test]
    fn epp_each_iterates_hash_with_two_params() {
        let template = "<% $h.each |$k, $v| { -%>\n<%= $k %>=<%= $v %>\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([(
            "h".to_string(),
            PuppetValue::Hash(IndexMap::from([(
                "name".to_string(),
                PuppetValue::String("nginx".to_string()),
            )])),
        )]));
        assert_eq!(render_epp(template, &params), "name=nginx\n");
    }

    #[test]
    fn epp_each_with_nested_if_emits_per_element() {
        let template =
            "<% $xs.each |$x| { -%>\n<% if $x { %>on<% } else { %>off<% } %>\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([(
            "xs".to_string(),
            PuppetValue::Array(vec![PuppetValue::Bool(true), PuppetValue::Bool(false)]),
        )]));
        assert_eq!(render_epp(template, &params), "on\noff\n");
    }

    #[test]
    fn epp_each_in_skipped_branch_emits_nothing() {
        let template = "<% if $on { -%>\n<% $xs.each |$x| { -%><%= $x %><% } -%>\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([
            ("on".to_string(), PuppetValue::Bool(false)),
            (
                "xs".to_string(),
                PuppetValue::Array(vec![PuppetValue::String("a".to_string())]),
            ),
        ]));
        assert_eq!(render_epp(template, &params), "");
    }

    #[test]
    fn epp_if_with_comparison_renders_branch_and_trailing_block() {
        // The original bug: a condition with an operator (`!=`) was treated as a
        // single missing variable, so the if-body *and* anything after it that
        // depended on the same flow vanished. Both should now render.
        let template = "<%= $ha_env %>\n<% if $ha_env != '' { -%>\nHA=on\n<% } -%>\n<% $extra.each |$e| { -%>\n- <%= $e %>\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([
            (
                "ha_env".to_string(),
                PuppetValue::String("prod".to_string()),
            ),
            (
                "extra".to_string(),
                PuppetValue::Array(vec![
                    PuppetValue::String("x".to_string()),
                    PuppetValue::String("y".to_string()),
                ]),
            ),
        ]));
        assert_eq!(render_epp(template, &params), "prod\nHA=on\n- x\n- y\n");
    }

    #[test]
    fn epp_if_comparison_false_skips_only_its_branch() {
        let template = "<% if $env == 'prod' { -%>\nP\n<% } else { -%>\nQ\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([(
            "env".to_string(),
            PuppetValue::String("dev".to_string()),
        )]));
        assert_eq!(render_epp(template, &params), "Q\n");
    }

    #[test]
    fn epp_if_numeric_and_logical_conditions() {
        let template = "<% if $n > 2 and $on { -%>\nyes\n<% } -%>\n";
        let yes = PuppetValue::Hash(IndexMap::from([
            ("n".to_string(), PuppetValue::Integer(5)),
            ("on".to_string(), PuppetValue::Bool(true)),
        ]));
        assert_eq!(render_epp(template, &yes), "yes\n");
        let no = PuppetValue::Hash(IndexMap::from([
            ("n".to_string(), PuppetValue::Integer(1)),
            ("on".to_string(), PuppetValue::Bool(true)),
        ]));
        assert_eq!(render_epp(template, &no), "");
    }

    #[test]
    fn epp_if_negation_and_parens() {
        let template = "<% if !($a == $b) { -%>\ndiff\n<% } -%>\n";
        let params = PuppetValue::Hash(IndexMap::from([
            ("a".to_string(), PuppetValue::String("1".to_string())),
            ("b".to_string(), PuppetValue::String("2".to_string())),
        ]));
        assert_eq!(render_epp(template, &params), "diff\n");
    }

    #[test]
    fn array_plus_concatenates_arrays() {
        let left = PuppetValue::Array(vec![PuppetValue::Integer(1)]);
        let right = PuppetValue::Array(vec![PuppetValue::Integer(2), PuppetValue::Integer(3)]);
        let result = eval_arith(&left, ArithOp::Add, &right);
        assert_eq!(
            result,
            PuppetValue::Array(vec![
                PuppetValue::Integer(1),
                PuppetValue::Integer(2),
                PuppetValue::Integer(3),
            ])
        );
    }

    #[test]
    fn array_plus_scalar_appends_element() {
        let left = PuppetValue::Array(vec![PuppetValue::String("a".to_string())]);
        let right = PuppetValue::String("b".to_string());
        let result = eval_arith(&left, ArithOp::Add, &right);
        assert_eq!(
            result,
            PuppetValue::Array(vec![
                PuppetValue::String("a".to_string()),
                PuppetValue::String("b".to_string()),
            ])
        );
    }

    #[test]
    fn unicode_in_pp_comments_does_not_break_parser() {
        // Em-dashes and other non-ASCII characters in `#` comments must not
        // break tokenization. Real-world modules frequently contain unicode
        // punctuation in comments and the parser should ignore it.
        let manifest = "\
class foo {
  # comment with em-dash — and unicode: café · ★ ñü
  # another line — second em-dash
  notify { 'hello': }
}
";
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("manifest with unicode in comments must parse");
        assert!(catalog.contains("notify", "hello"));
    }

    #[test]
    fn create_resources_instantiates_in_module_define() {
        // `create_resources('mod::api_key', $hash)` should iterate the hash
        // and instantiate the defined type once per key, expanding the body
        // into the catalog. Currently regent silently drops the call, so
        // api_key.pp's body shows 0% coverage and contain_exec assertions
        // against the inner resources fail.
        let manifest = r#"
            define foo::api_key(String $secret) {
              exec { "rotate-${title}":
                command => "/usr/bin/rotate ${secret}",
              }
            }
            class foo {
              $keys = {
                'prod' => { 'secret' => 'p' },
                'dev'  => { 'secret' => 'd' },
              }
              create_resources('foo::api_key', $keys)
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with create_resources must evaluate");
        assert!(
            catalog.contains("foo::api_key", "prod"),
            "create_resources should instantiate foo::api_key[prod]"
        );
        assert!(
            catalog.contains("foo::api_key", "dev"),
            "create_resources should instantiate foo::api_key[dev]"
        );
        assert!(
            catalog.contains("exec", "rotate-prod"),
            "child exec resource of defined type must be exposed in catalog"
        );
        assert!(
            catalog.contains("exec", "rotate-dev"),
            "child exec resource of defined type must be exposed in catalog"
        );
    }

    #[test]
    fn hash_each_block_iterates_and_declares_resources() {
        // `$hash.each |$key, $value| { ... }` is a common pattern in Puppet
        // manifests. The body must run once per hash entry with `$key`/`$value`
        // bound, declaring resources into the catalog. Previously the parser
        // saw `$var` with no `=` and silently dropped the statement.
        let manifest = r#"
            class foo {
              $items = {
                'one' => '/tmp/one',
                'two' => '/tmp/two',
              }
              $items.each |$name, $path| {
                file { $path:
                  ensure  => file,
                  content => $name,
                }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with hash .each must evaluate");
        assert!(catalog.contains("file", "/tmp/one"));
        assert!(catalog.contains("file", "/tmp/two"));
        let one = catalog.find("file", "/tmp/one").unwrap();
        assert_eq!(
            one.attributes.get("content"),
            Some(&PuppetValue::String("one".to_string()))
        );
    }

    #[test]
    fn array_each_block_iterates_with_single_param() {
        let manifest = r#"
            class foo {
              $names = ['a', 'b', 'c']
              $names.each |$n| {
                notify { $n: }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with array .each must evaluate");
        assert!(catalog.contains("notify", "a"));
        assert!(catalog.contains("notify", "b"));
        assert!(catalog.contains("notify", "c"));
    }

    #[test]
    fn class_parameter_is_readable_as_scoped_variable_cross_class() {
        // A class parameter must be reachable from another class as
        // `$class::param`, exactly like a body-assigned variable. Previously
        // only body variables were published under their qualified name, so a
        // cross-class read of a parameter resolved to Undef.
        let manifest = r#"
            class ferrogate($user = 'svc') {
              $config_dir = '/etc/ferrogate'
              include ferrogate::config
            }
            class ferrogate::config {
              file { $ferrogate::config_dir: ensure => directory }
              file { "/home/${ferrogate::user}": ensure => directory }
            }
        "#;
        let dir = write_module("ferrogate", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "ferrogate",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        assert!(
            catalog.contains("file", "/etc/ferrogate"),
            "cross-class body variable must resolve"
        );
        assert!(
            catalog.contains("file", "/home/svc"),
            "cross-class class parameter must resolve"
        );
    }

    #[test]
    fn each_on_literal_array_creates_resources() {
        // `.each` on a literal array (not a `$variable`) must still iterate and
        // declare the resources in its body. Previously the statement parser
        // had no branch for a leading `[`, so the loop body was dropped.
        let manifest = r#"
            class foo {
              ['a', 'b'].each |$n| {
                notify { $n: }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        assert!(catalog.contains("notify", "a"));
        assert!(catalog.contains("notify", "b"));
    }

    #[test]
    fn each_on_parenthesized_hash_literal_creates_resources() {
        // Puppet can't iterate a *bare* `{...}` hash literal at statement start
        // (ambiguous with a resource expression); the documented workaround is
        // to parenthesize it. `({...}).each |$k, $v| { … }` must iterate.
        let manifest = r#"
            class foo {
              ({ 'a' => 1, 'b' => 2 }).each |$k, $v| {
                notify { $k: }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        assert!(catalog.contains("notify", "a"));
        assert!(catalog.contains("notify", "b"));
    }

    #[test]
    fn class_resource_declaration_exposes_attributes() {
        // `class { 'foo': param => val }` must keep `param` on the catalog
        // resource so matchers can introspect it — expanding the class body
        // must not clobber the declared attributes.
        let manifest = r#"
            class foo($greeting = 'default') {
              notify { 'inner': }
            }
            class baseapp {
              class { 'foo': greeting => 'hi' }
            }
        "#;
        let dir = write_module("baseapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "baseapp",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        // Body still expanded.
        assert!(catalog.contains("notify", "inner"));
        let resource = catalog.find("class", "foo").expect("class[foo] in catalog");
        assert_eq!(
            resource.attributes.get("greeting"),
            Some(&PuppetValue::String("hi".to_string())),
            "declared class attribute must be introspectable"
        );
    }

    #[test]
    fn child_exec_of_in_module_define_is_in_catalog() {
        // Declaring a defined type inline must expand the body and add the
        // inner resources to the catalog so that `contain_exec(...)` can see
        // them — not just the wrapping define resource.
        let manifest = r#"
            define foo::app_secret(String $value) {
              exec { "install-${title}":
                command => "/usr/bin/install ${value}",
              }
            }
            class foo {
              foo::app_secret { 'primary': value => 'abc' }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class invoking a defined type must evaluate");
        assert!(catalog.contains("foo::app_secret", "primary"));
        assert!(
            catalog.contains("exec", "install-primary"),
            "child exec of defined type must be exposed via contain_exec"
        );
    }

    #[test]
    fn defined_type_resource_exposes_defaulted_params() {
        // A defined type declared inside a class exposes ALL its parameters on
        // the catalog resource — including ones left at their declared default.
        // `contain_dockerapp__run('web').with_ports([...])` must see the default
        // `ports` value instead of reading it back as Undef.
        let manifest = r#"
            define dockerapp::run(
              String $image,
              Array $ports = ['80:80'],
            ) {
              notify { "run-${title}": }
            }
            class profile {
              dockerapp::run { 'web': image => 'nginx' }
            }
        "#;
        let dir = write_module("dockerapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "profile",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class declaring a defined type must evaluate");
        let resource = catalog
            .find("dockerapp::run", "web")
            .expect("dockerapp::run[web] must be in the catalog");
        assert_eq!(
            resource.attributes.get("ports"),
            Some(&PuppetValue::Array(vec![PuppetValue::String(
                "80:80".to_string()
            )])),
            "defaulted `ports` must read back as its default, not Undef"
        );
        assert_eq!(
            resource.attributes.get("image"),
            Some(&PuppetValue::String("nginx".to_string())),
            "explicitly-passed `image` must still be present"
        );
    }

    #[test]
    fn subject_define_resource_exposes_defaulted_params() {
        // When the defined type is itself the test subject, its own catalog
        // resource must carry resolved parameters (passed + defaulted) so
        // `contain_<type>(title).with_<param>(...)` matches.
        let manifest = r#"
            define dockerapp::run(
              String $image,
              Array $ports = ['8080:80'],
            ) {
              notify { "run-${title}": }
            }
        "#;
        let dir = write_module("dockerapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut params = IndexMap::new();
        params.insert(
            "image".to_string(),
            PuppetValue::String("nginx".to_string()),
        );
        let catalog = evaluator
            .evaluate_define(
                "dockerapp::run",
                "web",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(params),
            )
            .expect("defined type subject must evaluate");
        let resource = catalog
            .find("dockerapp::run", "web")
            .expect("subject define dockerapp::run[web] must be in the catalog");
        assert_eq!(
            resource.attributes.get("ports"),
            Some(&PuppetValue::Array(vec![PuppetValue::String(
                "8080:80".to_string()
            )])),
            "defaulted `ports` on the subject define must read back as its default"
        );
        assert_eq!(
            resource.attributes.get("image"),
            Some(&PuppetValue::String("nginx".to_string())),
            "passed `image` param must be present on the subject define resource"
        );
    }

    #[test]
    fn subject_define_body_resources_are_in_catalog() {
        // When a defined type is the test subject, its body must expand into the
        // catalog (not just its own resource with parameters).
        let manifest = r#"
            define dockerapp::run(String $image) {
              notify { "run-${title}": }
              file { "/etc/${title}.conf": ensure => file }
            }
        "#;
        let dir = write_module("dockerapp", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let mut params = IndexMap::new();
        params.insert(
            "image".to_string(),
            PuppetValue::String("nginx".to_string()),
        );
        let catalog = evaluator
            .evaluate_define(
                "dockerapp::run",
                "web",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(params),
            )
            .unwrap();
        assert!(catalog.contains("notify", "run-web"), "body notify missing");
        assert!(
            catalog.contains("file", "/etc/web.conf"),
            "body file missing"
        );
    }

    #[test]
    fn selector_arm_may_hold_a_lambda() {
        // `subject ? { default => $xs.map |$x| { ... } }` must parse — the
        // selector body's braces must not suppress lambda recognition.
        let mut parser = PuppetParser::new(
            "class foo {\n  $a = $c ? { default => [1, 2].map |$i| { $i } }\n}\n",
        );
        let defs = parser
            .parse_definitions()
            .expect("selector arm with lambda must parse");
        assert_eq!(defs.len(), 1);
        // Control: the same lambda as an assignment RHS already parsed.
        let mut control = PuppetParser::new("class foo {\n  $b = [1, 2].map |$i| { $i } }\n");
        control
            .parse_definitions()
            .expect("assignment-RHS lambda must parse");
    }

    #[test]
    fn map_lambda_in_selector_arm_evaluates() {
        // The lambda isn't just parsed — `.map` actually transforms the array,
        // so a resource title built from it is correct.
        let manifest = r#"
            class foo {
              $relabel = 'z'
              $volumes = ['a', 'b']
              $mapped = $relabel ? {
                'none'  => $volumes,
                default => $volumes.map |$v| { "${v}:${relabel}" },
              }
              $mapped.each |$m| {
                notify { $m: }
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        assert!(catalog.contains("notify", "a:z"));
        assert!(catalog.contains("notify", "b:z"));
    }

    #[test]
    fn map_lambda_in_resource_attribute_value() {
        // Lambdas must also attach inside resource attribute values.
        let manifest = r#"
            class foo {
              $ports = ['80', '443']
              notify { 'x':
                message => $ports.map |$p| { "port-${p}" },
              }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        let resource = catalog.find("notify", "x").expect("notify[x] present");
        assert_eq!(
            resource.attributes.get("message"),
            Some(&PuppetValue::Array(vec![
                PuppetValue::String("port-80".to_string()),
                PuppetValue::String("port-443".to_string()),
            ])),
            "map lambda in attribute value must evaluate"
        );
    }

    #[test]
    fn ensure_packages_with_hash_arg_does_not_truncate_class_body() {
        // Regression: the `{ ensure => present }` argument to `ensure_packages`
        // used to leave the parser mid-call, so the hash's closing `}` was
        // mistaken for the class body's `}` — dropping every statement after the
        // call. Both the declared packages and the trailing `notify` must land.
        let manifest = r#"
            class foo {
              ensure_packages(['nginx', 'curl'], { 'ensure' => 'present' })
              notify { 'after': }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with ensure_packages must evaluate");
        assert!(
            catalog.contains("package", "nginx"),
            "package[nginx] declared"
        );
        assert!(
            catalog.contains("package", "curl"),
            "package[curl] declared"
        );
        assert!(
            catalog.contains("notify", "after"),
            "statement after ensure_packages must not be truncated"
        );
        let nginx = catalog.find("package", "nginx").unwrap();
        assert_eq!(
            nginx.attributes.get("ensure"),
            Some(&PuppetValue::String("present".to_string())),
            "shared params apply to each package"
        );
    }

    #[test]
    fn ensure_packages_array_defaults_ensure_present() {
        let manifest = r#"
            class foo {
              ensure_packages(['vim'])
              notify { 'after': }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .unwrap();
        assert!(catalog.contains("notify", "after"));
        let vim = catalog
            .find("package", "vim")
            .expect("package[vim] declared");
        assert_eq!(
            vim.attributes.get("ensure"),
            Some(&PuppetValue::String("present".to_string())),
            "ensure_packages defaults ensure => present"
        );
    }

    #[test]
    fn ensure_resource_with_hash_arg_does_not_truncate_class_body() {
        let manifest = r#"
            class foo {
              ensure_resource('package', 'htop', { 'ensure' => 'installed' })
              notify { 'after': }
            }
        "#;
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        let catalog = evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("class with ensure_resource must evaluate");
        assert!(
            catalog.contains("notify", "after"),
            "statement after ensure_resource must not be truncated"
        );
        let htop = catalog
            .find("package", "htop")
            .expect("package[htop] declared");
        assert_eq!(
            htop.attributes.get("ensure"),
            Some(&PuppetValue::String("installed".to_string())),
            "ensure_resource params apply to the declared resource"
        );
    }

    /// Evaluate `class foo { … }` with empty facts and params.
    fn eval_class_foo(manifest: &str) -> PuppetCatalog {
        let dir = write_module("foo", manifest);
        let evaluator = PuppetEvaluator::new(dir.path()).unwrap();
        evaluator
            .evaluate_class(
                "foo",
                &PuppetValue::Hash(IndexMap::new()),
                &PuppetValue::Hash(IndexMap::new()),
            )
            .expect("manifest must parse and evaluate")
    }

    #[test]
    fn empty_function_reports_collection_emptiness() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $blank = []
                  $full = ['x']
                  if empty($blank) { file { '/etc/empty-yes': ensure => file } }
                  if empty($full)  { file { '/etc/empty-no':  ensure => file } }
                  if empty('')     { file { '/etc/empty-str': ensure => file } }
                }
            "#,
        );
        assert!(catalog.contains("file", "/etc/empty-yes"));
        assert!(catalog.contains("file", "/etc/empty-str"));
        assert!(!catalog.contains("file", "/etc/empty-no"));
    }

    #[test]
    fn length_function_is_comparable() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $word = 'abcd'
                  if length($word) > 3  { file { '/etc/len-gt': ensure => file } }
                  if length($word) == 4 { file { '/etc/len-eq': ensure => file } }
                  if length($word) < 3  { file { '/etc/len-lt': ensure => file } }
                }
            "#,
        );
        assert!(catalog.contains("file", "/etc/len-gt"));
        assert!(catalog.contains("file", "/etc/len-eq"));
        assert!(!catalog.contains("file", "/etc/len-lt"));
    }

    #[test]
    fn string_type_match_enforces_length_bounds() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $name = 'hello'
                  if $name =~ String[1,10] { file { '/etc/inbounds':  ensure => file } }
                  if $name =~ String[1,3]  { file { '/etc/outbounds': ensure => file } }
                  if $name =~ String[5]    { file { '/etc/minonly':   ensure => file } }
                }
            "#,
        );
        assert!(catalog.contains("file", "/etc/inbounds"));
        assert!(catalog.contains("file", "/etc/minonly"));
        assert!(
            !catalog.contains("file", "/etc/outbounds"),
            "a 5-char string must not match String[1,3]"
        );
    }

    #[test]
    fn integer_enum_and_variant_type_matching() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $svc = 'running'
                  if 5 =~ Integer[1,10]                  { file { '/etc/int-ok':   ensure => file } }
                  if 50 =~ Integer[1,10]                 { file { '/etc/int-bad':  ensure => file } }
                  if $svc =~ Enum['stopped','running']   { file { '/etc/enum-ok':  ensure => file } }
                  if $svc =~ Enum['stopped','disabled']  { file { '/etc/enum-bad': ensure => file } }
                  if $svc =~ Variant[Integer, String[1]] { file { '/etc/variant':  ensure => file } }
                }
            "#,
        );
        assert!(catalog.contains("file", "/etc/int-ok"));
        assert!(!catalog.contains("file", "/etc/int-bad"));
        assert!(catalog.contains("file", "/etc/enum-ok"));
        assert!(!catalog.contains("file", "/etc/enum-bad"));
        assert!(catalog.contains("file", "/etc/variant"));
    }

    #[test]
    fn hash_indexing_with_integer_and_variable_keys() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $data = { 5 => 'five', 'name' => 'bob' }
                  $key = 5
                  if $data[5] == 'five'      { file { '/etc/intlit':  ensure => file } }
                  if $data[$key] == 'five'   { file { '/etc/intvar':  ensure => file } }
                  if $data['name'] == 'bob'  { file { '/etc/strkey':  ensure => file } }
                }
            "#,
        );
        assert!(
            catalog.contains("file", "/etc/intlit"),
            "$hash[5] must resolve an integer literal key"
        );
        assert!(
            catalog.contains("file", "/etc/intvar"),
            "$hash[$k] must evaluate the key variable rather than freeze its name"
        );
        assert!(catalog.contains("file", "/etc/strkey"));
    }

    #[test]
    fn array_indexing_by_position_and_variable() {
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $items = ['a', 'b', 'c']
                  $i = 0
                  $last = 0 - 1
                  if $items[1] == 'b'     { file { '/etc/lit':  ensure => file } }
                  if $items[$i] == 'a'    { file { '/etc/var':  ensure => file } }
                  if $items[$last] == 'c' { file { '/etc/neg':  ensure => file } }
                }
            "#,
        );
        assert!(catalog.contains("file", "/etc/lit"));
        assert!(catalog.contains("file", "/etc/var"));
        assert!(
            catalog.contains("file", "/etc/neg"),
            "negative array indices should count from the end"
        );
    }

    #[test]
    fn interpolated_array_uses_puppet_programmatic_form() {
        // Puppet renders an interpolated array with bracket delimiters and the
        // `%p` form for its members: string elements are double-quoted, while
        // numbers are left bare. The old behaviour stringified members with
        // `as_string`, leaking `[a, b]` (unquoted) into the catalog.
        let catalog = eval_class_foo(
            r#"
                class foo {
                  $strings = ['a', 'b']
                  $numbers = [1, 2]
                  notify { "s-${strings}": }
                  notify { "n-${numbers}": }
                }
            "#,
        );
        assert!(
            catalog.contains("notify", "s-[\"a\", \"b\"]"),
            "string array members must be double-quoted"
        );
        assert!(
            catalog.contains("notify", "n-[1, 2]"),
            "numeric array members must stay unquoted"
        );
    }

    #[test]
    fn resource_reference_still_parses_after_type_support() {
        // A capitalized *resource* type with a bracketed title must remain a
        // resource reference, not be mistaken for a data type.
        let catalog = eval_class_foo(
            r#"
                class foo {
                  file { '/etc/dep': ensure => file }
                  file { '/etc/needs-dep':
                    ensure  => file,
                    require => File['/etc/dep'],
                  }
                }
            "#,
        );
        let dependent = catalog
            .find("file", "/etc/needs-dep")
            .expect("file[/etc/needs-dep] declared");
        assert_eq!(
            dependent.attributes.get("require"),
            Some(&PuppetValue::String("File[/etc/dep]".to_string())),
            "File['/etc/dep'] must stay a resource reference"
        );
    }
}
